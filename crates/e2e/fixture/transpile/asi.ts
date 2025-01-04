a
interface A {}
/a/


a; () => {
    interface A {}
    /a/
}

var a = x as X
(1)

var a = () => x as X
(1)

f(() => 1)


if (1) type A = string


if (1) type A = string
else { type A = string }


if (1) type A = string
else type A = string

while (1) type A = string
for (;;) type A = string
for (var a in {}) type A = string
for (var a of []) type A = string

class A {
    a = 1
    abstract b
    ['c'](){}
}

class A {
    a = 1
    private ['c'](){}
}

class A {
    a = 1
    private *c(){}
}
