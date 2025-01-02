export enum Basic {
    A,
    B = 3,
    C,
    D = C + 3,
}

export enum Merge {
    A = 1,
}

export enum Merge {
    B = A + 2,
    // C = (() => {
    //     enum Merge {
    //         B = typeof A
    //     }
    //     return Merge.B
    // })()
}

export enum Merge {
    D = A + B
}

export enum Identifiers {
    "validIdentifier",
    "InvalidIdentifier\n",
    validIdentifierWithEscape\u0073
}

export enum Identifier\u0073 {
    A = validIdentifier + 10,
    B = validIdentifierWithEscapes + 11
}

export enum NameShadowing {
    NameShadowing,
    A = NameShadowing + 3
}

export enum NameShadowing {
    B = NameShadowing + 4
}

export declare enum ExportDeclare {
    A
}

export declare const enum ExportDeclareConst {
    A
}

export const enum ExportConst {
    A
}
