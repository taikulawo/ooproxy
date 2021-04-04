use std::ops::Range;

fn peek(data: &[u8], len_range: Range<usize>) -> Result<&[u8], &'static str> {
    let len_in_bits = data
        .get(len_range.clone())
        .ok_or("no enough data length to decode{}")?;
    let mut actual_len = 0usize;
    for bit in len_in_bits {
        actual_len = actual_len << 8 | (*bit as usize)
    }
    data.get(len_range.end .. len_range.end +  actual_len).ok_or("error when get index")
}
fn truncate_before(data: &[u8], len_range: Range<usize>) -> Result<&[u8], &'static str>{
    let len = peek(data, len_range.clone())?.len();
    Ok(&data[len_range.end + len ..])
}


pub struct TlsParser {

}

impl TlsParser {
    
}
pub fn parse_client_hello(data: &[u8]) -> Result<TlsParser, &'static str>{
    Ok(TlsParser{

    })
}