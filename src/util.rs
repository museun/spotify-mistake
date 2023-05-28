use std::future::Future;

pub enum Either<L, R> {
    Left(L),
    Right(R),
}

pub async fn select2<L, R>(left: L, right: R) -> Either<L::Output, R::Output>
where
    L: Future + Unpin + Send,
    R: Future + Unpin + Send,
{
    tokio::select! {
        left = left => Either::Left(left),
        right = right => Either::Right(right)
    }
}

pub fn format_duration(s: u32) -> String {
    let s = s / 1000;
    let (h, m, s) = (s / (60 * 60), (s / 60) % 60, s % 60);
    if h > 0 {
        return format!("{h:02}:{m:02}:{s:02}");
    }
    format!("{m:02}:{s:02}")
}
