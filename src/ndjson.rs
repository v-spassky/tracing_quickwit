use serde::Serialize;
use std::io;

pub(crate) fn serialize<W, V>(mut writer: W, value: &V) -> io::Result<()>
where
    W: io::Write,
    V: ?Sized + Serialize,
{
    serde_json::to_writer(&mut writer, value)?;
    writer.write_all(b"\n")
}
