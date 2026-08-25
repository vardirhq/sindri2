# The Decay language reference

Decay is a small, statically-typed scripting language for gameplay in the Sindri
Next engine. This document describes the language **as it actually behaves
today**, verified against the implementation rather than the intent.

It is written to be read by people and by AI assistants. Both make the same
mistake with a young language: assuming it works like the mature one it
resembles. Decay looks like Rust. It is much smaller than Rust. Where something
does not exist, this document says so explicitly rather than leaving it out —
see [What does not exist](#what-does-not-exist) and
[Surprising behaviour](#surprising-behaviour), and treat those two sections as
the most important ones here.

**Status: unstable.** The syntax, type model, IR, and runtime are all expected
to change. Nothing here is a compatibility promise.

---

## Contents

- [A complete example](#a-complete-example)
- [Lexical structure](#lexical-structure)
- [Grammar](#grammar)
- [Types](#types)
- [Containers](#containers)
- [Fields](#fields)
- [Functions](#functions)
- [Statements](#statements)
- [Expressions](#expressions)
- [Scope](#scope)
- [The host boundary](#the-host-boundary)
- [Execution limits](#execution-limits)
- [Surprising behaviour](#surprising-behaviour)
- [What does not exist](#what-does-not-exist)
- [Diagnostics](#diagnostics)

---

## A complete example

Every Decay feature that exists, in one file.

```rust
// Comments run to the end of the line. There are no block comments.

script PlayerController {
    // An authored property. `let` means the script never reassigns it;
    // `@export` means the host may set it before the script starts.
    @export
    let speed: f32 = 6.0;

    @export
    let label: String = "player";

    // Instance state. `var` may be reassigned. It survives between calls and
    // is not written back to the scene.
    var elapsed: f32 = 0.0;
    var airborne: bool = false;

    // Runs once, before the first `update`, after authored properties land.
    fn start() {
        elapsed = 0.0;
    }

    // Runs once a frame. `dt` is the frame's delta in seconds.
    fn update(dt: f32) {
        elapsed += dt;

        // A local binding. `let` is fixed, `var` may be reassigned.
        let offset: f32 = wave(elapsed) * speed;

        // Host paths. Decay does not know what a transform is; the host does.
        this.transform.position.x = offset;

        if offset > 0.0 {
            airborne = true;
        } else {
            airborne = false;
        }
    }

    // Functions may call each other by bare name, and may recurse.
    fn wave(seconds: f32) -> f32 {
        return sin(seconds);
    }
}
```

---

## Lexical structure

### Comments

`// to end of line`. That is the only comment form. **There are no `/* */` block
comments** and no doc comments.

### Identifiers

ASCII only: a letter or `_`, followed by letters, digits, or `_`. No Unicode
identifiers, and no raw identifiers.

### Keywords

```text
script  component  fn  let  var  if  else  return  true  false  null
```

All eleven are reserved. There are no contextual keywords.

### Number literals

```text
0        1        42        6.0       0.25
```

Digits, optionally followed by `.` and more digits. **There is no** exponent
notation (`1e6`), hex or binary (`0xff`), digit separator (`1_000`), leading dot
(`.5`), trailing dot (`1.`), or numeric suffix (`1.0f32`).

A negative number is the unary `-` operator applied to a literal, not part of
the literal.

Every number is the same type whether it is written `7` or `7.0`.

### String literals

Double quotes. Escapes: `\n`, `\r`, `\t`, `\"`, `\\`. Any other escape is a
diagnostic, and the character is taken literally. There are no raw strings, no
multi-line strings, no interpolation, and no single-quoted characters.

### Operators and punctuation

```text
+   -   *   /
+=  -=  *=  /=  =
==  !=  <   <=  >   >=
&&  ||  !
->  @   .   ,   ;   :
(   )   {   }   [   ]
```

`[` and `]` are recognised by the lexer but appear nowhere in the grammar; there
is no indexing and there are no array literals.

---

## Grammar

```ebnf
program      = { item } ;
item         = ( "script" | "component" ) IDENT "{" { member } "}" ;
member       = { attribute } ( field | function ) ;
attribute    = "@" IDENT ;

field        = ( "let" | "var" ) IDENT [ ":" type ] [ "=" expr ] ";" ;
function     = "fn" IDENT "(" [ params ] ")" [ "->" type ] block ;
params       = param { "," param } ;
param        = IDENT [ ":" type ] ;
type         = IDENT ;

block        = "{" { stmt } "}" ;
stmt         = binding | return | if | block | expr ";" ;
binding      = ( "let" | "var" ) IDENT [ ":" type ] [ "=" expr ] ";" ;
return       = "return" [ expr ] ";" ;
if           = "if" expr block [ "else" block ] ;

expr         = assign ;
assign       = binary [ ( "=" | "+=" | "-=" | "*=" | "/=" ) assign ] ;
binary       = unary { binop unary } ;         (* see the precedence table *)
unary        = [ "-" | "!" ] unary | postfix ;
postfix      = primary { "." IDENT | "(" [ args ] ")" } ;
args         = expr { "," expr } ;
primary      = IDENT | NUMBER | STRING | "true" | "false" | "null"
             | "(" expr ")" ;
```

Note that `if` takes a **block**, not a statement: `if x > 0.0 { }`, never
`if x > 0.0 doThing();`. Parentheses around the condition are allowed but do
nothing — they parse as an ordinary grouping expression — so write
`if x > 0.0 { }` rather than `if (x > 0.0) { }`.

Attributes are permitted only on fields. An attribute on a function is a
diagnostic.

---

## Types

| Written | Meaning |
| --- | --- |
| `f32` | The only numeric type |
| `bool` | `true` or `false` |
| `String` or `string` | Text |
| `unit` or `void` | No value; the default return type |
| anything else | A **named host type**, opaque to Decay |

There is no `i32`, no `u32`, no `f64`, and no integer type of any kind. `7` and
`7.0` are the same value, and `7 / 2` is `3.5`.

> **`f32` holds an `f64`.** The type is spelled `f32` because engine transforms
> are `f32`, but every Decay value is stored as a 64-bit float and narrows only
> when it crosses into the engine. This is a known inconsistency and an open
> decision, recorded in `docs/decay-direction.md`.

A named type such as `Transform` is opaque to the language: Decay has no way to
declare one. The **host** may describe it, and whether it has decides what
happens after a dot:

- **Described** — members are checked. `this.transform.positon.x` is a compile
  error naming the type and the member, and the member's type is enforced like
  any other, so assigning a number to a `bool` field is caught.
- **Not described** — members are `Unknown`, and unknown is compatible with
  everything, so anything after the dot is accepted and a mistake surfaces at
  runtime.

The rule is **per type**, which is what makes describing a host gradual: a host
part-way through describing itself does not reject scripts working against the
parts it has not reached. Describing `Transform` while leaving `RigidBody`
undescribed means `this.transform.positon` is caught and
`this.rigidbody.anything.at.all` is not.

`this` follows the same rule. Once the host describes anything on `this`, a
member of `this` it did not describe is a compile error — so against Sindri,
which offers only `transform`, `this.sprite` is refused. What Sindri describes
is in `docs/scripting.md`.

`null` may be assigned to a named type, and to nothing else.

### Host references

A value of a named host type can be **held**, not only reached through. A host
may hand one back from a call, and a script can bind it, keep it in a field,
pass it, and compare it:

```decay
let target = World.find("Player");
if target != null && target != this.entity {
    target.transform.position.x = 0.0;
}
```

What a script cannot do is look inside one. There is no literal for a reference,
no arithmetic on one, and no conversion in either direction; the only references
a script holds are ones the host gave it. Whether a reference still names
anything is the host's question to answer — the language has no opinion on
whether the thing behind one is alive, and against Sindri that is
`World.exists`.

This is what a reference is *for*: it is the difference between a script that
can only describe itself and one that can say something about another thing in
the world. `docs/scripting.md` records what Sindri makes of it.

---

## Containers

A file holds any number of `script` and `component` declarations. Names must be
unique across the file.

```rust
script Enemy { }
component Health { }
```

Both parse identically and hold the same members. The distinction is carried
through to the IR as `ContainerKind` for the host to act on; **the language
itself treats them the same**, and `sindri-decay` currently instantiates scripts
only.

---

## Fields

```rust
let speed: f32 = 6.0;      // fixed after initialization
var elapsed: f32 = 0.0;    // reassignable
@export let jump: f32 = 8.0;
```

A field needs a type, an initializer, or both. With neither, it is a diagnostic.

A field with no initializer starts as `null`.

Field initializers run in **declaration order**, and may read fields declared
**above** them:

```rust
let base: f32 = 2.0;
let doubled: f32 = base * 2.0;   // fine
```

Reading a field declared *below* compiles and then fails at runtime with
`UnknownPath`. The analyzer does not catch this yet.

### `@export`

`@export` marks a field as authored by the host. It is the only attribute that
exists.

The host may set an exported field regardless of `let` or `var`: `let` means the
*script* does not reassign it, not that the author cannot author it. This is the
`[SerializeField]` distinction from other engines, and it is the capability that
justified Decay being statically typed — see `docs/scripting.md`.

A host that sets a field which is not `@export`, or does not exist, is refused.

---

## Functions

```rust
fn name(a: f32, b: bool) -> f32 { return 0.0; }
fn no_return(dt: f32) { }
```

An omitted return type is `unit`. An omitted parameter type is unknown, which is
compatible with everything — annotate parameters.

A function with no `return` returns unit. `return;` with no value also returns
unit.

Functions in the same container call each other **by bare name**:

```rust
fn helper() -> f32 { return 1.0; }
fn go() -> f32 { return helper(); }     // correct
```

> **`this.helper()` is not a method call.** There are no methods on a container.
> It used to become a *host path call* named `this.helper` and fail at runtime
> with `FunctionNotFound`, which looked like the engine's fault; it is now a
> compile error saying to write `helper(...)` instead. Always call sibling
> functions by bare name.

A **host type** may have methods, and those are checked: if the host describes
`RigidBody` with an `add_impulse` taking two numbers, then
`this.rigidbody.add_impulse(0.0, 1.0)` is checked for arity and argument types
like any other call.

Recursion works, bounded by the call-depth limit.

### `this`

`this` is bound in every function to the container's own type. Two uses work:

- `this.<field>` reads or writes one of the container's own fields. `speed` and
  `this.speed` are the same field.
- `this.<anything else>` is a **host path** — `this.transform.position.x` means
  nothing to Decay and everything to the host.

### Lifecycle

The language has no lifecycle of its own; the host decides which functions it
calls. `sindri-decay` calls `start()` once and `update(dt)` each frame, both
optional and both with exact signatures. See `docs/scripting.md`.

---

## Statements

```rust
let x: f32 = 1.0;      // binding, fixed
var y: f32 = 2.0;      // binding, reassignable
y = 3.0;               // expression statement
return y;              // return
return;                // return unit
if y > 0.0 { } else { }
{ }                    // bare block, its own scope
```

A binding needs a type, an initializer, or both.

`if` requires a `bool` condition — there is no truthiness, so `if x` where `x`
is a number is a diagnostic. `else if` is written as `else { if ... }`; a
chained `else if` is **not** supported by the grammar.

---

## Expressions

### Precedence

Loosest to tightest:

| Level | Operators | Associativity |
| --- | --- | --- |
| 1 | `=` `+=` `-=` `*=` `/=` | right |
| 2 | `\|\|` | left |
| 3 | `&&` | left |
| 4 | `==` `!=` | left |
| 5 | `<` `<=` `>` `>=` | left |
| 6 | `+` `-` | left |
| 7 | `*` `/` | left |
| 8 | `-` `!` (unary prefix) | right |
| 9 | `.` `()` (postfix) | left |

### Operand rules

- `+ - * /` require `f32` on both sides and produce `f32`. **`+` does not
  concatenate strings.**
- `< <= > >=` require `f32` and produce `bool`.
- `== !=` require the two sides to be compatible types, and produce `bool`.
- `&& ||` require `bool` on both sides and produce `bool`, and **short-circuit**:
  the right side is evaluated only when the left does not already decide the
  answer. So `held != null && World.exists(held)` guards the call to its right,
  and `ready || expensive()` does not ask when it is already ready.
- unary `-` requires `f32`; unary `!` requires `bool`.

### Assignment

`=` assigns. `+= -= *= /=` read, apply, and assign, and require `f32` on both
sides.

Assignment targets are a name or a member path. Assigning to a `let` binding, to
a function, or to anything else is a diagnostic.

Assignment is an expression and evaluates to the assigned value, so `a = b = 1.0`
works.

### Division by zero

`1.0 / 0.0` is infinity, not an error. There is no integer division to trap.

---

## Scope

Function parameters and the function body share one scope, so a parameter and a
top-level binding of the same name collide.

Every `if` branch and every bare block opens a nested scope. A binding inside one
leaves with it, and shadows rather than replaces an outer binding of the same
name:

```rust
var x: f32 = 1.0;
if flag {
    var x: f32 = 2.0;   // a different x
}
// x is still 1.0
```

Assigning to a name the block did not declare writes through to the outer one,
as expected.

Container fields are visible in every function of that container.

---

## The host boundary

Decay has **no built-in functions and no standard library**. Not even `print` —
where one exists, the host registered it.

Everything a script can name beyond its own container comes from the host, which
registers globals before compilation. The compiler knows their names and
signatures and nothing else, and the runtime forwards every access as a path.

An unresolved name is a compile error. An unresolved *path* is a compile error
too, when the host described the type it goes through, and a runtime error when
it did not.

A host describes a type by listing its members, and describes what `this` offers
beyond the script's own fields. A container's own field always wins over a host
member of the same name, so the engine growing a name can never shadow state a
script already had.

For what the Sindri engine specifically provides — the entity's transform and
sprite, the keyboard, the frame's time, `print`, and six maths functions — see
`docs/scripting.md`. That document is the whole list; this one never grows a
Sindri-specific name, because the language does not have one.

---

## Execution limits

Decay calls may nest 64 deep by default, after which the script fails with
`CallDepthExceeded`. The host may change the limit.

**There is no operation budget**, because there are no loops: recursion is
currently the only way to run forever, and the depth limit bounds it. A budget
becomes necessary the moment loops exist.

---

## Surprising behaviour

Things that are true and that most readers — human or model — will guess wrong.

1. **`this.method()` is not a method call.** A container has no methods; it is a
   compile error telling you to call it by bare name. Host *types* may have
   methods, and those work.
2. **There is no `else if`.** Write `else { if ... }`.
3. **There is no truthiness.** `if x` requires `x` to be `bool`.
4. **`+` does not join strings.** It is numeric addition only.
5. **All numbers are floats.** `7 / 2` is `3.5`. There is no integer type and no
   integer division.
6. **Member types are checked only where the host described them.**
   `this.transfrom.position.x` is now a compile error against Sindri, because
   Sindri describes its transform — but a path into a type nobody described is
   still accepted and still fails at runtime.
7. **A field cannot read a field declared below it.** It compiles and fails at
   runtime.
8. **`let` fields are still settable by the host.** That is what `@export` means.
9. **A parameter shadows nothing** — parameters and body bindings share one
   scope, so reusing a parameter's name is a duplicate, not a shadow.

## What does not exist

Do not write these. They are not unimplemented corners; they are absent from the
grammar, and every one of them is a parse error or a diagnostic.

**Control flow:** `while`, `for`, `loop`, `break`, `continue`, `match`, `else if`,
ternaries, labelled blocks.

**Data:** arrays, lists, maps, dictionaries, tuples, structs, enums, indexing
(`a[0]`), ranges (`0..3`), `Option`, `Result`, `?`.

**Functions:** closures, lambdas, function values, default arguments, named
arguments, variadics, generics, overloading, methods, `impl`, traits.

**Types:** integers, `f64`, unsigned types, characters, type aliases, casts,
inference beyond a binding's own initializer, nullable types, user-defined types
beyond `script` and `component`.

**Modules:** `import`, `use`, `mod`, `pub`, visibility of any kind, multiple
files. One file is one compilation unit and cannot refer to another.

**Standard library:** `print`, `math.*`, string methods, formatting,
interpolation, conversion functions, collections, iteration, time, randomness.

**Other:** operator overloading, macros, attributes other than `@export`, block
comments, doc comments, `const`, `static`, exceptions, `try`, `panic`,
concurrency, `async`.

---

## Diagnostics

Compilation runs in two phases and reports both together, each diagnostic
carrying a byte span plus a 1-based line and column.

**Syntax** diagnostics come from the lexer and parser. The parser recovers and
continues, so one missing semicolon does not hide the rest of the file.

**Semantic** diagnostics come from name resolution and type checking: unknown
names, type mismatches, assignment to an immutable binding, duplicate members or
locals, a non-`bool` `if` condition, and wrong argument counts.

**A program with any diagnostic does not lower.** There is no partial
compilation and no warning level — everything reported is fatal.

Runtime failures are values, not panics: `UnknownPath`, `Immutable`,
`FunctionNotFound`, `Arity`, `InvalidBinary`, `CallDepthExceeded`, and others.
The host decides what to do with them; `sindri-decay` reports them per entity and
keeps running every other script.
