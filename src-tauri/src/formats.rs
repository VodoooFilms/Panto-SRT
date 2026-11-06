//! SRT and FCPXML exporters.

use crate::grouping::Clip;

pub fn seconds_to_srt(seconds: f64) -> String {
    let total = (seconds * 1000.0).round().max(0.0) as u64;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        total / 3_600_000,
        (total % 3_600_000) / 60_000,
        (total % 60_000) / 1000,
        total % 1000
    )
}

fn escape_line(text: &str) -> String {
    let cleaned = text.replace('\r', "");
    // Collapse 3+ newlines to 2, like the original implementation.
    let mut out = String::with_capacity(cleaned.len());
    let mut newlines = 0;
    for ch in cleaned.chars() {
        if ch == '\n' {
            newlines += 1;
            if newlines <= 2 {
                out.push(ch);
            }
        } else {
            newlines = 0;
            out.push(ch);
        }
    }
    out.trim().to_string()
}

pub fn build_srt(clips: &[Clip], offset: f64) -> String {
    let body = clips
        .iter()
        .filter(|c| !c.text.trim().is_empty())
        .enumerate()
        .map(|(i, clip)| {
            format!(
                "{}\n{} --> {}\n{}\n",
                i + 1,
                seconds_to_srt(clip.start + offset),
                seconds_to_srt(clip.end + offset),
                escape_line(&clip.text)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{}\n", body.trim_end())
}

pub fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn rational(seconds: f64, fps: u32) -> String {
    let ticks = if seconds.is_finite() {
        (seconds.max(0.0) * fps as f64).round() as u64
    } else {
        0
    };
    format!("{ticks}/{fps}s")
}

fn project_name_from_path(file_path: &str) -> String {
    let base = file_path.rsplit(['/', '\\']).next().unwrap_or("");
    let stem = match base.rfind('.') {
        Some(pos) if pos > 0 => &base[..pos],
        _ => base,
    };
    let stem = if stem.is_empty() { "Subtítulos" } else { stem };
    format!("Panto SRT · {stem}")
}

pub fn build_fcpxml(clips: &[Clip], fps: u32, timecode_base: f64, source_path: &str) -> String {
    let fps = fps.clamp(1, 120);
    let cues: Vec<(f64, f64, String)> = clips
        .iter()
        .filter(|c| !c.text.trim().is_empty())
        .map(|clip| {
            let duration = (clip.end - clip.start).max(0.1);
            (clip.start, duration, clip.text.trim().replace('\n', "<br/>"))
        })
        .collect();

    let sequence_duration = cues
        .last()
        .map(|(start, duration, _)| start + duration)
        .unwrap_or(0.0);
    let project_name = escape_xml(&project_name_from_path(source_path));

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<!DOCTYPE fcpxml SYSTEM \"https://developer.apple.com/DTDs/fcpxml-1.9.dtd\">\n");
    out.push_str("<fcpxml version=\"1.9\">\n");
    out.push_str("  <resources>\n");
    out.push_str(&format!(
        "    <format id=\"r1\" name=\"FFVideoFormat1080p{fps}\" frameDuration=\"1/{fps}s\" width=\"1920\" height=\"1080\"/>\n"
    ));
    out.push_str("  </resources>\n");
    out.push_str("  <library>\n");
    out.push_str("    <event name=\"Panto SRT\">\n");
    out.push_str(&format!("      <project name=\"{project_name}\">\n"));
    out.push_str(&format!(
        "        <sequence duration=\"{}\" format=\"r1\" tcStart=\"{}\" tcFormat=\"NDF\">\n",
        rational(sequence_duration, fps),
        rational(timecode_base, fps)
    ));
    out.push_str("          <spine>\n");
    for (index, (start, duration, text)) in cues.iter().enumerate() {
        out.push_str(&format!(
            "            <title name=\"{}\" lane=\"1\" offset=\"{}\" duration=\"{}\">\n",
            escape_xml(&format!("Subtítulo {}", index + 1)),
            rational(*start, fps),
            rational(*duration, fps)
        ));
        out.push_str("              <text>\n");
        out.push_str(&format!(
            "                <text-style-def id=\"sts-{index}\"><text-style font-family=\"Helvetica Neue\" font-size=\"42\" alignment=\"center\" fillColor=\"1 1 1 1\"><stroke width=\"3\" color=\"0 0 0 1\"/></text-style></text-style-def>\n"
        ));
        out.push_str(&format!(
            "                <text-style ref=\"sts-{index}\">{}</text-style>\n",
            escape_xml(text)
        ));
        out.push_str("              </text>\n");
        out.push_str("            </title>\n");
    }
    out.push_str("          </spine>\n");
    out.push_str("        </sequence>\n");
    out.push_str("      </project>\n");
    out.push_str("    </event>\n");
    out.push_str("  </library>\n");
    out.push_str("</fcpxml>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(start: f64, end: f64, text: &str) -> Clip {
        Clip { index: 0, start, end, text: text.into() }
    }

    #[test]
    fn srt_timestamp() {
        assert_eq!(seconds_to_srt(0.0), "00:00:00,000");
        assert_eq!(seconds_to_srt(65.5), "00:01:05,500");
        assert_eq!(seconds_to_srt(3661.007), "01:01:01,007");
    }

    #[test]
    fn srt_numbered_with_offset() {
        let srt = build_srt(&[clip(0.0, 1.25, "Hola"), clip(2.0, 3.5, "Mundo")], 3600.0);
        assert_eq!(srt, "1\n01:00:00,000 --> 01:00:01,250\nHola\n\n2\n01:00:02,000 --> 01:00:03,500\nMundo\n");
    }

    #[test]
    fn srt_skips_empty() {
        let srt = build_srt(&[clip(0.0, 1.0, "ok"), clip(1.0, 2.0, "   ")], 0.0);
        assert_eq!(srt, "1\n00:00:00,000 --> 00:00:01,000\nok\n");
    }

    #[test]
    fn fcpxml_spine_and_tcstart() {
        let xml = build_fcpxml(&[clip(0.0, 1.2, "Hola <mundo> & amigos")], 25, 3600.0, "/tmp/demo.mp4");
        assert!(xml.contains("<title name=\"Subtítulo 1\" lane=\"1\" offset=\"0/25s\" duration=\"30/25s\">"));
        assert!(xml.contains("tcStart=\"90000/25s\""));
        assert!(xml.contains("Hola &lt;mundo&gt; &amp; amigos"));
        assert!(xml.contains("<project name=\"Panto SRT · demo\">"));
        assert!(xml.ends_with("</fcpxml>\n"));
    }

    #[test]
    fn fcpxml_rounds_to_frames() {
        let xml = build_fcpxml(&[clip(0.3333, 1.9999, "bien")], 30, 0.0, "/tmp/x.mp4");
        assert!(xml.contains("offset=\"10/30s\""));
        assert!(xml.contains("duration=\"50/30s\""));
    }
}
