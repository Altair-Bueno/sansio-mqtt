/// Produces owned [`bytes::Bytes`] for a slice taken from this input.
///
/// [`crate::Payload`], [`crate::BinaryData`] and [`crate::Utf8String`] all
/// store [`bytes::Bytes`], so decoding them requires turning a borrowed slice
/// of the input into an owned buffer. Doing that with
/// [`bytes::Bytes::copy_from_slice`] costs one allocation and one
/// `memcpy` per field, which for a large PUBLISH means copying the
/// whole application message on every receive.
///
/// When the caller already owns the frame as [`bytes::Bytes`], that
/// copy is avoidable: [`bytes::Bytes::slice_ref`] turns a subslice of an
/// owned buffer into a reference-counted view of it. Wrap the input in
/// [`winnow::stream::Stateful`], carrying the owning buffer as the
/// state, and the leaf parsers below will share it instead of copying:
///
/// ```ignore
/// let frame: bytes::Bytes = /* the received frame */;
/// let mut input = Stateful {
///     input: Partial::new(&frame[..]),
///     state: &frame,
/// };
/// let packet = ControlPacket::parser::<_, DecodeError, DecodeError>(&settings)
///     .parse_next(&mut input)?;
/// ```
///
/// Passing a plain `&[u8]` still works and still copies, so this is an
/// opt-in optimisation rather than a required change.
pub trait BytesSource {
    /// Returns `slice` as owned [`bytes::Bytes`].
    ///
    /// `slice` is expected to be a subslice of this input. Implementors
    /// that cannot prove that MUST copy rather than panic.
    fn owned_slice(&self, slice: &[u8]) -> bytes::Bytes;
}

/// Copies: a bare slice carries no ownership to share.
impl BytesSource for &[u8] {
    #[inline]
    fn owned_slice(&self, slice: &[u8]) -> bytes::Bytes {
        bytes::Bytes::copy_from_slice(slice)
    }
}

impl<Input> BytesSource for winnow::stream::Partial<Input>
where
    Input: BytesSource,
{
    #[inline]
    fn owned_slice(&self, slice: &[u8]) -> bytes::Bytes {
        // `Partial` only tracks whether more input may follow, so defer
        // to the buffer it wraps.
        <Input as BytesSource>::owned_slice(self, slice)
    }
}

/// Returns a reference-counted view when `slice` really does point into
/// the owning buffer, and copies otherwise.
///
/// The range check keeps this total:
/// [`bytes::Bytes::slice_ref`] panics when handed a slice from a
/// different allocation, which would otherwise turn a caller mistake
/// (state not matching input) into a panic inside the parser.
#[inline]
fn owned_slice_of(owner: &bytes::Bytes, slice: &[u8]) -> bytes::Bytes {
    if slice.is_empty() {
        return bytes::Bytes::new();
    }

    let owner_start = owner.as_ptr() as usize;
    let slice_start = slice.as_ptr() as usize;
    let within_owner = slice_start >= owner_start
        && slice_start
            .checked_add(slice.len())
            .is_some_and(|slice_end| slice_end <= owner_start + owner.len());

    if within_owner {
        owner.slice_ref(slice)
    } else {
        bytes::Bytes::copy_from_slice(slice)
    }
}

impl<Input> BytesSource for winnow::stream::Stateful<Input, &bytes::Bytes> {
    #[inline]
    fn owned_slice(&self, slice: &[u8]) -> bytes::Bytes {
        owned_slice_of(self.state, slice)
    }
}

impl<Input> BytesSource for winnow::stream::Stateful<Input, bytes::Bytes> {
    #[inline]
    fn owned_slice(&self, slice: &[u8]) -> bytes::Bytes {
        owned_slice_of(&self.state, slice)
    }
}
