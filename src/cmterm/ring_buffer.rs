

#[derive(Clone)]
pub struct RingBuffer<T, const S: usize>
    where T : Default + Clone
{
    inner: [T; S],
    position: usize
}

impl<T, const S: usize> RingBuffer<T, S>
    where T : Default + Clone
{
    pub fn new() -> RingBuffer<T, S> {
        return RingBuffer { 
            inner: std::array::from_fn::<T, S, _>(|_| { T::default() }),
            position: 0
        }
    }

    pub fn push(&mut self, element: T) {
        self.position = (self.position + 1) % S;
        self.inner[self.position] = element;
    }

    /// Return a Vec containing references to the last n values in the buffer
    pub fn peek_last_n(&self, n: usize) -> Vec<&T> {
        debug_assert!(n <= S, "Attempted to peek more than entire ring buffer!");

        let mut peek_buf = Vec::<&T>::with_capacity(n);

        for i in 0..n {
            let i = (self.position.overflowing_sub(i).0) % S;

            peek_buf.push(&self.inner[i]);
        }

        return peek_buf;
    }
}