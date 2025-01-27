declare global {
    export var a: string
}

export namespace A {
    export const variable = 1;
    export function Function() { }
    export class Class { }
    export namespace Empty { }
    export namespace Ambient {
        export interface A {}
    }

    // @D
    // export class Decorated1 { }

    // export @D class Decorated2 { }

    const X = { a: 1, b: 2, c: 3, d: 4, e: 5, f: { g: 6 }, h: 7 };
    var e;
    export const { a, b: c, d = ({ e } = X), f: { g }, ...h } = X
    export const { nested = (() => {
        namespace A {
            export const a = 'a';
        }
        return A;
    }) } = {}

    export function Overloaded(): void;
    export function Overloaded(b?: string): void {}

    export declare function DeclaredFunction()
    export declare class DeclaredClass { }
    export declare const DeclaredVariable = 'declared';
    export interface Interface { }

    namespace Unexported {
        export const z = 1;
    }

    export namespace B.C {
        export var D = 123;
    }
}

export namespace Foo.Bar {
    export let Baz = 321;
}

export namespace Foo2.Bar.Baz {
    export let Baz = 321;
}

export module Module {
    export let hello = 42;
}


export namespace NameShadowing {
    const NameShadowing = 'NameShadowing';
    export const a = NameShadowing;
}


export namespace NameShadowingExported {
    export const NameShadowingExported = 'NameShadowingExported';
    export const a = NameShadowingExported.length;
}

export namespace Empty { }
export namespace Ambient {
    export interface A {}
}
