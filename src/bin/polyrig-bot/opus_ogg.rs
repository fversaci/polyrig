/**************************************************************************
  Copyright 2026 Francesco Versaci (https://github.com/fversaci/)

  This program is free software: you can redistribute it and/or modify
  it under the terms of the GNU Affero General Public License as published by
  the Free Software Foundation, either version 3 of the License, or
  (at your option) any later version.

  This program is distributed in the hope that it will be useful,
  but WITHOUT ANY WARRANTY; without even the implied warranty of
  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
  GNU Affero General Public License for more details.

  You should have received a copy of the GNU Affero General Public License
  along with this program.  If not, see <https://www.gnu.org/licenses/>.
**************************************************************************/
//! Encodes raw PCM audio as an Ogg/Opus stream for Telegram voice messages.

use anyhow::{Result, bail};
use ogg::{PacketWriteEndInfo, PacketWriter};
use opus::{Application, Bitrate, Channels, Encoder};

const SAMPLE_RATE: u32 = 24_000;
const CHANNELS: Channels = Channels::Mono;
const BITRATE: i32 = 48_000;
// Valid Opus frame sizes in samples at 24 kHz (2.5 to 20 ms), longest first.
const FRAME_SIZES: [usize; 4] = [480, 240, 120, 60];
const MIN_FRAME: usize = 60;
const MAX_PACKET_BYTES: usize = 2_048;
// 50 x 20 ms packets per page keeps the muxing delay low enough for players
// to start playback while a page is still downloading.
const PACKETS_PER_PAGE: usize = 50;
const VENDOR: &[u8] = b"polyrig";

/// An Ogg/Opus voice message payload plus its duration in whole seconds.
pub struct VoiceOgg {
    pub data: Vec<u8>,
    pub duration_secs: u32,
}

/// Encodes raw little-endian 16-bit mono PCM at 24 kHz into an Ogg/Opus
/// stream, the format Telegram expects for voice messages.
pub fn pcm_to_ogg_opus(pcm: &[u8]) -> Result<VoiceOgg> {
    let samples: Vec<i16> = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&chunk| i16::from_le_bytes(chunk))
        .collect();
    if samples.is_empty() {
        bail!("empty PCM input");
    }

    let mut encoder = Encoder::new(SAMPLE_RATE, CHANNELS, Application::Voip)?;
    encoder.set_bitrate(Bitrate::Bits(BITRATE))?;
    // Ogg/Opus granule positions and the pre-skip field are always expressed
    // at 48 kHz, while the lookahead is reported at the encoder input rate.
    let rate_scale: u64 = u64::from(48_000 / SAMPLE_RATE);
    let pre_skip = (encoder.get_lookahead()?.max(0) as u64) * rate_scale;

    let serial = rand::random::<u32>();
    let mut out = Vec::new();
    {
        let mut writer = PacketWriter::new(&mut out);
        writer.write_packet(opus_head(pre_skip), serial, PacketWriteEndInfo::EndPage, 0)?;
        writer.write_packet(opus_tags(), serial, PacketWriteEndInfo::EndPage, 0)?;

        let mut packet = [0u8; MAX_PACKET_BYTES];
        let mut page_packets = 0usize;
        let mut fed = 0usize;
        let mut idx = 0usize;
        while idx < samples.len() {
            let remaining = samples.len() - idx;
            let mut padded = [0i16; MIN_FRAME];
            let frame: &[i16] = if remaining >= MIN_FRAME {
                let size = FRAME_SIZES
                    .iter()
                    .copied()
                    .find(|&s| s <= remaining)
                    .unwrap_or(MIN_FRAME);
                &samples[idx..idx + size]
            } else {
                padded[..remaining].copy_from_slice(&samples[idx..]);
                &padded
            };
            let n = encoder.encode(frame, &mut packet)?;
            fed += frame.len();
            idx += frame.len();
            let end = if idx >= samples.len() {
                PacketWriteEndInfo::EndStream
            } else {
                page_packets += 1;
                if page_packets == PACKETS_PER_PAGE {
                    page_packets = 0;
                    PacketWriteEndInfo::EndPage
                } else {
                    PacketWriteEndInfo::NormalPacket
                }
            };
            let absgp = pre_skip + (fed as u64) * rate_scale;
            writer.write_packet(packet[..n].to_vec(), serial, end, absgp)?;
        }
    }

    let duration_secs = (((samples.len() as u64) * rate_scale + 24_000) / 48_000) as u32;
    Ok(VoiceOgg {
        data: out,
        duration_secs,
    })
}

fn opus_head(pre_skip: u64) -> Vec<u8> {
    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead");
    head.push(1); // format version
    head.push(1); // mono
    head.extend_from_slice(&(pre_skip as u16).to_le_bytes());
    head.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    head.extend_from_slice(&0i16.to_le_bytes()); // output gain
    head.push(0); // channel mapping family
    head
}

fn opus_tags() -> Vec<u8> {
    let mut tags = Vec::with_capacity(8 + 4 + VENDOR.len() + 4);
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&(VENDOR.len() as u32).to_le_bytes());
    tags.extend_from_slice(VENDOR);
    tags.extend_from_slice(&0u32.to_le_bytes()); // no comments
    tags
}
