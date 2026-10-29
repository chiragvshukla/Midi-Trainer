//! The Mutopia Project piano catalogue: browse it, and save a piece as MIDI.
//!
//! Mutopia (mutopiaproject.org) publishes public-domain and Creative Commons editions and
//! offers every piece as a `.mid`. This module fetches one page of its Piano listing, parses
//! the generated result tables into [`MutopiaPiece`] rows, and streams a chosen `.mid` into a
//! library folder. The network is only touched from an explicit user action — opening the
//! Download tab, searching, loading another page, or pressing Download.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

/// The Mutopia site, used to turn the page's relative links into absolute ones.
const SITE: &str = "https://www.mutopiaproject.org";

/// The listing CGI. `Instrument=Piano` is baked in; `startat` pages it.
const LIST_URL: &str = "https://www.mutopiaproject.org/cgibin/make-table.cgi";

/// Only MIDI files under this prefix are ever fetched, so a stray URL cannot pull anything
/// else. The listing always links absolute FTP URLs.
const FTP_PREFIX: &str = "https://www.mutopiaproject.org/ftp/";

/// How many rows the CGI returns per page. Used only to advance `startat`.
pub const PAGE_SIZE: usize = 10;

/// The markup that starts each piece's table, and the link that marks a further page.
const PIECE_MARKER: &str = "<table class=\"table-bordered result-table\">";
const NEXT_MARKER: &str = ">Next 10<";

/// One catalogued piece, as the download tab shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MutopiaPiece {
    /// The piece's Mutopia id, or 0 when the page carried no info link.
    pub id: u32,
    pub title: String,
    pub composer: String,
    /// The instruments the piece is written for, e.g. "Voice, Piano".
    pub instrumentation: String,
    /// The period or genre, e.g. "Romantic".
    pub style: String,
    /// The licence name as printed, e.g. "Public Domain".
    pub license: String,
    /// The licence's page on Mutopia.
    pub license_url: String,
    /// The single `.mid` to fetch; absent when the piece is a multi-file ZIP.
    pub midi_url: Option<String>,
    /// The ZIP of several `.mid` files, present only for multi-movement pieces.
    pub zip_url: Option<String>,
    /// The piece's Mutopia information page.
    pub info_url: String,
}

/// One page of the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MutopiaPage {
    pub pieces: Vec<MutopiaPiece>,
    /// Whether the listing has a page after this one.
    pub has_more: bool,
}

/// Why the catalogue could not be listed or a piece fetched.
#[derive(Debug)]
pub enum MutopiaError {
    /// The request failed or the server returned an error status.
    Http(Box<ureq::Error>),
    /// A response could not be read, or the file could not be written.
    Io(std::io::Error),
    /// A download URL was not a Mutopia MIDI file.
    BadUrl,
}

impl std::fmt::Display for MutopiaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MutopiaError::Http(err) => write!(f, "could not reach the Mutopia Project: {err}"),
            MutopiaError::Io(err) => write!(f, "could not save the file: {err}"),
            MutopiaError::BadUrl => write!(f, "that is not a Mutopia Project MIDI link"),
        }
    }
}

impl std::error::Error for MutopiaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MutopiaError::Http(err) => Some(err.as_ref()),
            MutopiaError::Io(err) => Some(err),
            MutopiaError::BadUrl => None,
        }
    }
}

impl From<ureq::Error> for MutopiaError {
    fn from(err: ureq::Error) -> Self {
        MutopiaError::Http(Box::new(err))
    }
}

impl From<std::io::Error> for MutopiaError {
    fn from(err: std::io::Error) -> Self {
        MutopiaError::Io(err)
    }
}

/// Lists one page of the Piano catalogue starting at row `start_at`, optionally filtered by the
/// free-text `query`.
pub fn list_piano(start_at: usize, query: &str) -> Result<MutopiaPage, MutopiaError> {
    let mut url = format!("{LIST_URL}?Instrument=Piano&startat={start_at}");
    let query = query.trim();
    if !query.is_empty() {
        url.push_str("&searchingfor=");
        url.push_str(&percent_encode(query));
    }
    let body = ureq::get(&url).call()?.into_string()?;
    let (pieces, has_more) = parse_page(&body);
    Ok(MutopiaPage { pieces, has_more })
}

/// Downloads the MIDI at `url` into `dest`, returning the saved file's path.
pub fn download_midi(url: &str, dest: &Path) -> Result<PathBuf, MutopiaError> {
    let url = url.trim();
    let name = midi_file_name(url).ok_or(MutopiaError::BadUrl)?;
    std::fs::create_dir_all(dest)?;
    let destination = dest.join(&name);
    // A `.part` file means an interrupted download never looks complete.
    let temporary = destination.with_extension("part");

    if let Err(err) = stream_to(url, &temporary) {
        let _ = std::fs::remove_file(&temporary);
        return Err(err);
    }
    std::fs::rename(&temporary, &destination)?;
    Ok(destination)
}

/// Streams `url` into `path`, creating or truncating it.
fn stream_to(url: &str, path: &Path) -> Result<(), MutopiaError> {
    let response = ureq::get(url).call()?;
    let mut reader = response.into_reader();
    let mut file = BufWriter::new(File::create(path)?);
    std::io::copy(&mut reader, &mut file)?;
    file.flush()?;
    Ok(())
}

/// Splits a listing page into its pieces and whether a further page exists.
fn parse_page(html: &str) -> (Vec<MutopiaPiece>, bool) {
    let has_more = html.contains(NEXT_MARKER);
    let mut pieces = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find(PIECE_MARKER) {
        let after = &rest[start + PIECE_MARKER.len()..];
        let end = after.find("</table>").unwrap_or(after.len());
        if let Some(piece) = parse_piece(&after[..end]) {
            pieces.push(piece);
        }
        rest = &after[end..];
    }
    (pieces, has_more)
}

/// Parses one piece's table body.
fn parse_piece(block: &str) -> Option<MutopiaPiece> {
    let cells = split_cells(block);
    let title = cell_text(cells.first().copied()?);
    if title.is_empty() {
        return None;
    }
    let composer = {
        let raw = cell_text(cells.get(1).copied().unwrap_or(""));
        raw.strip_prefix("by ").unwrap_or(&raw).trim().to_string()
    };
    let instrumentation = cells
        .iter()
        .map(|cell| cell_text(cell))
        .find_map(|text| {
            text.strip_prefix("for ")
                .map(|rest| rest.trim().to_string())
        })
        .unwrap_or_default();
    let style = cells.get(6).map(|cell| cell_text(cell)).unwrap_or_default();

    let license_cell = cells.iter().find(|cell| cell.contains("legal.html"));
    let license = license_cell.map(|cell| cell_text(cell)).unwrap_or_default();
    let license_url = license_cell
        .and_then(|cell| first_href(cell))
        .map(|href| resolve_license(&href))
        .unwrap_or_else(|| format!("{SITE}/legal.html"));

    let info_href = cells
        .iter()
        .filter_map(|cell| first_href(cell))
        .find(|href| href.contains("piece-info.cgi"));
    let id = info_href.as_deref().and_then(id_from_href).unwrap_or(0);
    let info_url = if id == 0 {
        String::new()
    } else {
        format!("{SITE}/cgibin/piece-info.cgi?id={id}")
    };

    let hrefs = all_hrefs(block);
    let midi_url = hrefs.iter().find(|href| href.ends_with(".mid")).cloned();
    let zip_url = hrefs
        .iter()
        .find(|href| href.ends_with("-mids.zip"))
        .cloned();

    Some(MutopiaPiece {
        id,
        title,
        composer,
        instrumentation,
        style,
        license,
        license_url,
        midi_url,
        zip_url,
        info_url,
    })
}

/// The inner text of every `<td>` in `block`, in order.
fn split_cells(block: &str) -> Vec<&str> {
    let mut cells = Vec::new();
    let mut rest = block;
    while let Some(open) = rest.find("<td") {
        let after = &rest[open + 3..];
        let Some(tag_end) = after.find('>') else {
            break;
        };
        let content = &after[tag_end + 1..];
        let Some(close) = content.find("</td>") else {
            break;
        };
        cells.push(&content[..close]);
        rest = &content[close + 5..];
    }
    cells
}

/// Every `href="…"` value in `text`, in order.
fn all_hrefs(text: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("href=\"") {
        let after = &rest[start + "href=\"".len()..];
        let Some(end) = after.find('"') else {
            break;
        };
        hrefs.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    hrefs
}

/// The first `href="…"` value in `text`.
fn first_href(text: &str) -> Option<String> {
    all_hrefs(text).into_iter().next()
}

/// A cell's visible text: tags stripped, entities decoded, whitespace collapsed.
fn cell_text(cell: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for ch in cell.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }
    decode_entities(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Decodes the handful of HTML entities the listing uses. `&amp;` is decoded last so an
/// escaped entity (`&amp;lt;`) is not double-decoded.
fn decode_entities(text: &str) -> String {
    let mut out = text.replace("&nbsp;", " ");
    for (entity, value) in [
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&apos;", "'"),
        ("&amp;", "&"),
    ] {
        out = out.replace(entity, value);
    }
    out
}

/// An absolute URL for a licence link such as `../legal.html#publicdomain`.
fn resolve_license(href: &str) -> String {
    match href.rsplit_once('#') {
        Some((_, fragment)) => format!("{SITE}/legal.html#{fragment}"),
        None => format!("{SITE}/legal.html"),
    }
}

/// The numeric id in a `piece-info.cgi?id=…` link.
fn id_from_href(href: &str) -> Option<u32> {
    let rest = href.rsplit_once("id=")?.1;
    let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// The safe file name of a Mutopia MIDI URL, or `None` when it is not one.
fn midi_file_name(url: &str) -> Option<String> {
    if !url.starts_with(FTP_PREFIX) || url.contains(char::is_whitespace) {
        return None;
    }
    let name = url.rsplit('/').next()?;
    let lower = name.to_ascii_lowercase();
    if name.is_empty() || lower == ".." || name.contains('\\') || !lower.ends_with(".mid") {
        return None;
    }
    Some(name.to_string())
}

/// Percent-encodes a search term for the CGI's query string.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        decode_entities, id_from_href, midi_file_name, parse_page, percent_encode, resolve_license,
    };

    /// A saved copy of a real listing page: five pieces, two of them multi-file ZIPs, ending
    /// in the "Next 10" link.
    const FIXTURE: &str = include_str!("../../test_data/mutopia_piano.html");

    #[test]
    fn parses_every_piece_on_a_catalogue_page() {
        let (pieces, has_more) = parse_page(FIXTURE);
        assert_eq!(pieces.len(), 5);
        assert!(has_more, "the fixture ends with a Next link");
    }

    #[test]
    fn a_page_without_a_next_link_is_the_last() {
        let last = FIXTURE.replace(">Next 10<", "");
        let (pieces, has_more) = parse_page(&last);
        assert_eq!(pieces.len(), 5);
        assert!(!has_more);
    }

    #[test]
    fn reads_a_normal_piece() {
        let (pieces, _) = parse_page(FIXTURE);
        let vocalise = &pieces[0];
        assert_eq!(vocalise.title, "Vocalise № 1");
        assert_eq!(vocalise.composer, "F. Abt (1819–1885)");
        assert_eq!(vocalise.instrumentation, "Voice, Piano");
        assert_eq!(vocalise.style, "Romantic");
        assert_eq!(vocalise.license, "Public Domain");
        assert_eq!(
            vocalise.license_url,
            "https://www.mutopiaproject.org/legal.html#publicdomain"
        );
        assert_eq!(vocalise.id, 2247);
        assert_eq!(
            vocalise.info_url,
            "https://www.mutopiaproject.org/cgibin/piece-info.cgi?id=2247"
        );
        assert_eq!(
            vocalise.midi_url.as_deref(),
            Some("https://www.mutopiaproject.org/ftp/AbtF/Vocalise1/Vocalise1.mid")
        );
        assert_eq!(vocalise.zip_url, None);
    }

    #[test]
    fn reads_a_piece_whose_cells_carry_attributes() {
        // Toccatina's download cell is `<td class="zipped">`, and its only ZIP is the LilyPond
        // source — not a MIDI archive — so it still has a single `.mid`.
        let (pieces, _) = parse_page(FIXTURE);
        let toccatina = pieces.iter().find(|p| p.title == "Toccatina").unwrap();
        assert_eq!(
            toccatina.license,
            "Creative Commons Attribution-ShareAlike 4.0"
        );
        assert_eq!(
            toccatina.midi_url.as_deref(),
            Some("https://www.mutopiaproject.org/ftp/AlkanCV/O75/toccatina/toccatina.mid")
        );
        assert_eq!(toccatina.zip_url, None);
    }

    #[test]
    fn a_multi_file_piece_has_a_zip_and_no_single_midi() {
        let (pieces, _) = parse_page(FIXTURE);
        let suite = pieces
            .iter()
            .find(|p| p.title.starts_with("French Suite no. 3"))
            .expect("the suite is listed");
        assert_eq!(suite.midi_url, None);
        assert_eq!(
            suite.zip_url.as_deref(),
            Some(
                "https://www.mutopiaproject.org/ftp/BachJS/BWV814/french-suite-3/french-suite-3-mids.zip"
            )
        );
    }

    #[test]
    fn decodes_the_entities_the_listing_uses() {
        let (pieces, _) = parse_page(FIXTURE);
        let fugue = pieces
            .iter()
            .find(|p| p.title.contains("Fugue"))
            .expect("the fugue is listed");
        assert_eq!(fugue.title, "Prelude & Fugue in C");
        assert_eq!(
            decode_entities("A &amp; B &lt;3&gt; &#39;x&#39;"),
            "A & B <3> 'x'"
        );
    }

    #[test]
    fn only_mutopia_midi_links_are_accepted() {
        let ok = "https://www.mutopiaproject.org/ftp/AbtF/Vocalise1/Vocalise1.mid";
        assert_eq!(midi_file_name(ok).as_deref(), Some("Vocalise1.mid"));
        assert!(midi_file_name("https://evil.example/ftp/x.mid").is_none());
        assert!(midi_file_name("https://www.mutopiaproject.org/ftp/x.txt").is_none());
        assert!(midi_file_name("https://www.mutopiaproject.org/ftp/").is_none());
        assert!(midi_file_name("https://www.mutopiaproject.org/ftp/..").is_none());
        assert!(midi_file_name("https://www.mutopiaproject.org/ftp/a b.mid").is_none());
        // Only the last path segment is ever used, so a name can never re-enter the path.
        assert_eq!(
            midi_file_name("https://www.mutopiaproject.org/ftp/../secret.mid").as_deref(),
            Some("secret.mid")
        );
    }

    #[test]
    fn reads_ids_and_licences() {
        assert_eq!(id_from_href("piece-info.cgi?id=2247"), Some(2247));
        assert_eq!(id_from_href("piece-info.cgi"), None);
        assert_eq!(
            resolve_license("../legal.html#ccasa"),
            "https://www.mutopiaproject.org/legal.html#ccasa"
        );
        assert_eq!(
            resolve_license("legal.html"),
            "https://www.mutopiaproject.org/legal.html"
        );
    }

    #[test]
    fn encodes_a_search_term() {
        assert_eq!(percent_encode("chopin nocturne"), "chopin%20nocturne");
        assert_eq!(percent_encode("Bach's"), "Bach%27s");
        assert_eq!(percent_encode("a-b_c.d~e"), "a-b_c.d~e");
    }
}
