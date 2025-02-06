// Just like ts-blank-space and swc
let a: string = "";

enum Foo {
  A = 1,
  B = A + 2,
//^^^^^^^^^ Positions of enum member initializaitons are preserved.
}

// Enum merging is supported
enum Foo {
  World = A
}


namespace Bar {
  export const A = 1;
//       ^^^^^^^^^^^ Positions of exported declarations are preserved.
}


class Baz {
  
}


enum A { a }
namespace B { export const b = 1 }
class C { constructor(public c) { } }


A<{ a: 1 & 2 }>(0)
A<{ a: 1 + 2 }>(0)

