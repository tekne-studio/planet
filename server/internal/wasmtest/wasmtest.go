// Package wasmtest writes WebAssembly by its bytes, for the tests of what
// hosts it: the server has no compiler of WebAssembly, and a module written
// to harm is said most plainly byte by byte.
package wasmtest

// Header is what every module opens with: `\0asm` and version 1.
const Header = "\x00asm\x01\x00\x00\x00"

// Leb is `n` as an unsigned LEB128.
func Leb(n uint32) []byte {
	var out []byte
	for {
		b := byte(n & 0x7f)
		n >>= 7
		if n == 0 {
			return append(out, b)
		}
		out = append(out, b|0x80)
	}
}

// I32 is `i32.const n`.
func I32(n int32) []byte {
	out := []byte{0x41}
	for {
		b := byte(n & 0x7f)
		n >>= 7
		if (n == 0 && b&0x40 == 0) || (n == -1 && b&0x40 != 0) {
			return append(out, b)
		}
		out = append(out, b|0x80)
	}
}

// Join is the parts one after the other.
func Join(parts ...[]byte) []byte {
	var out []byte
	for _, part := range parts {
		out = append(out, part...)
	}
	return out
}

// Vector is a count and the items, one after the other.
func Vector(items ...[]byte) []byte {
	return Join(Leb(uint32(len(items))), Join(items...))
}

// Section is a section of `id` holding `content`.
func Section(id byte, content []byte) []byte {
	return Join([]byte{id}, Leb(uint32(len(content))), content)
}

// Name is a name as a module writes one: its length, then its bytes.
func Name(name string) []byte { return Join(Leb(uint32(len(name))), []byte(name)) }

// Body is a function's body of no locals and the code given.
func Body(code []byte) []byte {
	code = Join([]byte{0x00}, code, []byte{0x0b}) // no locals, the code, end
	return Join(Leb(uint32(len(code))), code)
}

// Module is the header and the sections given.
func Module(sections ...[]byte) []byte {
	return Join([]byte(Header), Join(sections...))
}

// Cartridge is one that keeps the contract. Beside the contract's five it
// has `fillers` functions of no parameters, each of the code given. Its
// screen is at 4096 and its palette at 0.
func Cartridge(fillers int, filler []byte) []byte {
	const (
		ask  = 0 // () -> i32: cart_abi, frame, palette
		tell = 1 // (i32) -> (): init, update
		void = 2 // () -> (): a filler
	)
	types := Vector(
		[]byte{0x60, 0x00, 0x01, 0x7f},
		[]byte{0x60, 0x01, 0x7f, 0x00},
		[]byte{0x60, 0x00, 0x00},
	)
	kinds := [][]byte{{ask}, {tell}, {tell}, {ask}, {ask}}
	code := [][]byte{
		Body(I32(1)),    // cart_abi
		Body(nil),       // init
		Body(nil),       // update
		Body(I32(4096)), // frame
		Body(I32(0)),    // palette
	}
	for range fillers {
		kinds = append(kinds, []byte{void})
		code = append(code, Body(filler))
	}
	exports := [][]byte{Join(Name("memory"), []byte{0x02, 0x00})}
	for i, export := range []string{"cart_abi", "init", "update", "frame", "palette"} {
		exports = append(exports, Join(Name(export), []byte{0x00, byte(i)}))
	}
	return Module(
		Section(1, types),
		Section(3, Vector(kinds...)),
		Section(5, Vector([]byte{0x00, 0x01})),
		Section(7, Vector(exports...)),
		Section(10, Vector(code...)),
	)
}

// Loops is a filler of `n` empty loops and a call: preparing puts a meter
// at every loop and a count at every call.
func Loops(n int) []byte {
	var code []byte
	for range n {
		// loop; br_if 0 (i32.const 0); end
		code = append(code, 0x03, 0x40, 0x41, 0x00, 0x0d, 0x00, 0x0b)
	}
	// init (i32.const 0)
	return Join(code, I32(0), []byte{0x10, 0x01})
}

// Calls is a filler of `n` direct calls to the first filler, two bytes
// each: what preparing grows most for its size, a count at every call.
func Calls(n int) []byte {
	var code []byte
	for range n {
		code = append(code, 0x10, 0x05)
	}
	return code
}
