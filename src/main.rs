mod cava;
mod smoother;
mod mapper;
mod colorizer;
mod output;

use cava::{CavaConfig, CavaReader};
use smoother::Smoother;
use mapper::{build_frame_data, BarLayout};
use colorizer::ColorMode;
use output::{BarEmitter, BarOutput, write_output, write_eww_output};

use std::io::{self, BufWriter, Write};

// ─── Configuración ────────────────────────────────────────────────────────────

/// Número de canales. Debe coincidir con `bars` en ~/.config/cava/eww.ini.
const CHANNELS: usize = 16;

/// Parámetros de suavizado.
const ALPHA_RISE: f32 = 0.75;
const GRAVITY:    f32 = 0.025;

/// Umbral de silencio: amplitud por debajo de la cual un canal se considera mudo.
const SILENCE_THRESHOLD: f32 = 0.02;

/// Frames de silencio consecutivos antes de activar el estado `Silent`.
const SILENT_FRAMES_THRESHOLD: u32 = 45;

// ─── Main ─────────────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let eww_mode = args.iter().any(|a| a == "--ags" || a == "--eww");
    let color_mode = if args.iter().any(|a| a == "--led") {
        ColorMode::Led
    } else {
        ColorMode::ByAmplitude
    };

    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());

    let config    = CavaConfig::default_binary(CHANNELS);
    let mut reader    = CavaReader::new(io::stdin().lock(), config);
    let mut smoother  = Smoother::new(CHANNELS, ALPHA_RISE, GRAVITY);
    let mut emitter   = BarEmitter::new(color_mode, SILENT_FRAMES_THRESHOLD);

    let layout = BarLayout::Compact;

    loop {
        match reader.next_frame() {
            Err(_) => {
                let out = emitter.emit_error();
                emit(&mut writer, &out.text, &out, eww_mode)?;
                writer.flush()?;
            }

            Ok(None) => {
                let out = emitter.emit_error();
                emit(&mut writer, &out.text, &out, eww_mode)?;
                writer.flush()?;
                break;
            }

            Ok(Some(frame)) => {
                smoother.update(&frame.values);

                let is_silent  = frame.is_silent(SILENCE_THRESHOLD);
                let peak       = frame.peak();
                let frame_data = build_frame_data(smoother.values(), layout);

                let out = emitter.emit(&frame_data, peak, is_silent);
                emit(&mut writer, &out.text, &out, eww_mode)?;
                writer.flush()?;
            }
        }
    }

    Ok(())
}

fn emit<W: Write>(
    writer: &mut W,
    text: &str,
    output: &BarOutput,
    eww_mode: bool,
) -> io::Result<()> {
    if eww_mode {
        write_eww_output(writer, text)
    } else {
        write_output(writer, output)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use cava::{CavaConfig, CavaReader};
    use std::io::Cursor;

    fn make_i16_frame(channels: usize, value: i16) -> Vec<u8> {
        (0..channels).flat_map(|_| value.to_le_bytes()).collect()
    }

    #[test]
    fn pipeline_frame_activo() {
        let data   = make_i16_frame(CHANNELS, i16::MAX / 2);
        let config = CavaConfig::default_binary(CHANNELS);
        let mut reader   = CavaReader::new(Cursor::new(data), config);
        let mut smoother = Smoother::new(CHANNELS, ALPHA_RISE, GRAVITY);
        let mut emitter  = BarEmitter::new(ColorMode::ByAmplitude, 30);

        let frame      = reader.next_frame().unwrap().unwrap();
        smoother.update(&frame.values);
        let frame_data = build_frame_data(smoother.values(), BarLayout::Compact);
        let out        = emitter.emit(&frame_data, frame.peak(), frame.is_silent(SILENCE_THRESHOLD));

        assert!(out.text.contains("<span"));
        assert!(out.percentage.unwrap() > 0);
    }

    #[test]
    fn pipeline_eof_limpio() {
        let config = CavaConfig::default_binary(CHANNELS);
        let mut reader = CavaReader::new(Cursor::new(vec![]), config);
        assert!(reader.next_frame().unwrap().is_none());
    }
}
