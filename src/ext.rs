pub trait JoinWith<T>
where
    Self: IntoIterator<Item = T> + Sized,
    T: AsRef<str>,
{
    fn join(self, sep: &str) -> String {
        self.into_iter().fold(String::new(), |mut a, c| {
            if !a.is_empty() {
                a.push_str(sep);
            }
            a.push_str(c.as_ref());
            a
        })
    }
}

impl<T, I> JoinWith<T> for I
where
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
}
