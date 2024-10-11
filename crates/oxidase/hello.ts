type Flatten<Type> = Type extends Array<infer Item extends string>
  ? Item
  : Type;
