use winnow::stream::AsChar;
use winnow::stream::Compare;
use winnow::stream::Stream;
use winnow::stream::StreamIsPartial;
use winnow::token::take_till;
use winnow::PResult;
use winnow::Parser;

pub fn till_null<I>(input: &mut I) -> PResult<<I as Stream>::Slice>
where
    I: Stream + StreamIsPartial + Compare<char>,
    <I as Stream>::Token: AsChar,
    <I as Stream>::Token: AsChar,
{
    let ret = take_till(1.., '\0').parse_next(input)?;
    let _ = '\0'.parse_next(input)?;
    Ok(ret)
}
