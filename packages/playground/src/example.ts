let a: string = "";
interface A {

}

enum Foo {
  A = 1,
  B = A + 2,
}

// Enum merging
enum Foo {
  C = A
}

namespace Bar {
  export const A = 1;
}

class Baz {
  constructor(public a: string) {
    console.log(a);
  }
}
