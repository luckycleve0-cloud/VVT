//! Фрейминг сообщений: 4 байта длины (LE) + JSON.
//!
//! JSON выбран осознанно: на этом этапе важнее отлаживаемость (`wireshark`,
//! `nc`, глазами в логе), чем байты. Когда появится реальный трафик и
//! профилирование покажет узкое место — переедем на `postcard`/`bincode`,
//! сохранив тот же интерфейс.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt;
use std::io::{self, Read, Write};

/// Максимальный размер одного сообщения — защита от «злого» клиента.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// Ошибка кодирования/декодирования.
#[derive(Debug)]
pub enum CodecError {
    Io(io::Error),
    /// Сообщение больше лимита.
    TooLarge(usize),
    /// Некорректный JSON.
    Json(serde_json::Error),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "ошибка ввода-вывода: {err}"),
            Self::TooLarge(size) => write!(
                f,
                "сообщение слишком большое: {size} байт (лимит {MAX_FRAME_BYTES})"
            ),
            Self::Json(err) => write!(f, "некорректное сообщение: {err}"),
        }
    }
}

impl std::error::Error for CodecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Json(err) => Some(err),
            Self::TooLarge(_) => None,
        }
    }
}

impl From<io::Error> for CodecError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for CodecError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

/// Сериализовать сообщение в байты (без длины).
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, CodecError> {
    let bytes = serde_json::to_vec(msg)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(CodecError::TooLarge(bytes.len()));
    }
    Ok(bytes)
}

/// Разобрать байты в сообщение.
pub fn decode<T: DeserializeOwned>(payload: &[u8]) -> Result<T, CodecError> {
    Ok(serde_json::from_slice(payload)?)
}

/// Записать сообщение в поток вместе с заголовком длины.
pub fn write_frame<W: Write, T: Serialize>(writer: &mut W, msg: &T) -> Result<(), CodecError> {
    let payload = encode(msg)?;
    let len = u32::try_from(payload.len()).map_err(|_| CodecError::TooLarge(payload.len()))?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()?;
    Ok(())
}

/// Прочитать одно сообщение из потока.
///
/// `Ok(None)` — чистое закрытие соединения (клиент ушёл).
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Option<Vec<u8>>, CodecError> {
    // Заголовок читаем по байту, чтобы отличить «клиент ушёл» (0 байт)
    // от «соединение оборвалось на середине заголовка» (ошибка).
    let mut len_buf = [0u8; 4];
    let mut filled = 0usize;
    while filled < len_buf.len() {
        match reader.read(&mut len_buf[filled..]) {
            Ok(0) => {
                return if filled == 0 {
                    Ok(None)
                } else {
                    Err(CodecError::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "соединение оборвалось на середине заголовка",
                    )))
                };
            }
            Ok(read) => filled += read,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(CodecError::Io(err)),
        }
    }

    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > MAX_FRAME_BYTES {
        return Err(CodecError::TooLarge(len));
    }
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    Ok(Some(payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vtt_proto::{ClientMsg, PROTOCOL_VERSION};

    #[test]
    fn roundtrip_through_buffer() {
        let msg = ClientMsg::Hello {
            name: "Алиса".into(),
            code: "ABCDEF".into(),
            protocol: PROTOCOL_VERSION,
            resume: None,
        };
        let mut buf = Vec::new();
        write_frame(&mut buf, &msg).unwrap();
        assert!(buf.len() > 4);

        let mut cursor = std::io::Cursor::new(buf);
        let payload = read_frame(&mut cursor).unwrap().unwrap();
        let back: ClientMsg = decode(&payload).unwrap();
        assert_eq!(back, msg);
        // После сообщения — конец потока.
        assert!(read_frame(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn empty_stream_is_none() {
        let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
        assert!(read_frame(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn truncated_header_errors() {
        let mut cursor = std::io::Cursor::new(vec![1u8, 0]);
        let err = read_frame(&mut cursor).unwrap_err();
        assert!(matches!(err, CodecError::Io(_)));
    }

    #[test]
    fn oversized_frame_is_rejected() {
        let mut bytes = u32::try_from(MAX_FRAME_BYTES + 1)
            .unwrap()
            .to_le_bytes()
            .to_vec();
        bytes.extend_from_slice(&[0u8; 8]);
        let mut cursor = std::io::Cursor::new(bytes);
        let err = read_frame(&mut cursor).unwrap_err();
        assert!(matches!(err, CodecError::TooLarge(_)));
    }

    #[test]
    fn invalid_json_is_reported() {
        let err = decode::<ClientMsg>(b"{ not json }").unwrap_err();
        assert!(matches!(err, CodecError::Json(_)));
        assert!(err.to_string().contains("некорректное"));
    }

    #[test]
    fn zero_length_frame_rejected() {
        let mut cursor = std::io::Cursor::new(0u32.to_le_bytes().to_vec());
        assert!(read_frame(&mut cursor).is_err());
    }
}
