
// # Internals
// 
// After some testing, here is _probably_ what is happening:
// - All IO/TXHOLD/PEEK point to the same FIFOs (at least same tx for sure).
// - When you write to the TX FIFO, you can only write 4 entries that can be up to 32 bits long (or
//  24 in variable mode).
// - The FIFOs are 4 entries deep (with entries of any size).
// - The IO/TXHOLD/PEEK registers are 32 bits wide, forget about the "16 bits" that is said in the
//  documentation.
//
// ### STATUS Register
// - Bits 28-30: TX FIFO level (in bytes)
// - Bits 20-22: RX FIFO level (in bytes)


mod registers;
