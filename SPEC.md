# Avtan Language Specification

Status: draft 0.1

Avtan is a Rust-inspired systems language that transpiles to Go. It combines
explicit data ownership, algebraic data types, checked effects, refinement and
dependent types, and first-class concurrency primitives that compile to Go
goroutines, channels, contexts, wait groups, mutexes, and atomics.

This document is the initial language specification. It is intentionally
implementation-oriented: features are described together with their type-checking
rules and Go lowering strategy where that affects the public language design.

## 1. Design Goals

Avtan should:

1. Feel familiar to Rust users while producing readable, idiomatic Go output.
2. Make concurrency explicit, typed, and easier to audit than raw goroutines.
3. Provide full Idris-style dependent types through a small total core language,
   while keeping Rust-like surface syntax for dependent `struct` and `enum`
   declarations, equality proofs, implicit arguments, elaboration, and proof
   erasure.
4. Use ownership and borrowing for local safety, but map predictably to Go's
   garbage-collected runtime.
5. Preserve source-level invariants in generated Go through compile-time proofs,
   erased evidence, explicit runtime decisions, and generated wrapper types.
6. Prefer a small, well-specified core calculus over clever surface syntax in the
   first versions.

Avtan is not trying to be:

1. A drop-in Rust replacement.
2. A standalone proof assistant, even though it has a proof-capable type system.
3. A Go syntax variant.
4. A language that exposes unsafe pointer arithmetic as a normal programming
   model.

## 2. Compilation Model

The compiler pipeline is:

1. Parse Avtan source into an AST.
2. Resolve names, modules, imports, and type aliases.
3. Elaborate surface syntax into a dependently typed core with implicit
   arguments, metavariables, holes, erased arguments, and explicit binders.
4. Type-check core terms using normalization, definitional equality,
   higher-order unification where required by elaboration, and universe checking.
5. Check totality, termination, and coverage for definitions used in types or
   proofs.
6. Type-check ownership, effects, and concurrency capabilities for the runtime
   fragment.
7. Erase proofs, implicit-only terms, compile-time indices, and other
   non-runtime evidence.
8. Lower the erased runtime core to Go AST.
9. Format and emit Go modules.

The generated Go is part of the public contract. A user should be able to inspect
and debug it. Generated names may be mangled, but must be deterministic.

### 2.1 Target Go

The default backend target is Go 1.22 or newer. The compiler may support older
or newer target profiles through configuration.

Generated Go must avoid reflection unless a feature explicitly requires it.
Generated Go must not use `unsafe` unless an Avtan package opts into an `unsafe`
backend feature.

### 2.2 Source Files

Avtan source files use the extension `.avtn`.

Each file starts with an optional package declaration:

```avtan
package net.http
```

If omitted, the package is inferred from the directory.

## 3. Lexical Structure

### 3.1 Comments

```avtan
// line comment

/*
  block comment
*/
```

Documentation comments use `///` before items and `//!` before modules.

### 3.2 Identifiers

Identifiers are Unicode by source syntax, but the MVP compiler may restrict
public exported identifiers to ASCII letters, digits, and `_` for simpler Go
interop.

Naming conventions:

1. `snake_case` for variables, functions, modules, and fields.
2. `PascalCase` for types, traits, enum variants, and effects.
3. `SCREAMING_SNAKE_CASE` for constants.

### 3.3 Keywords

Reserved keywords:

```text
as async await borrow box break chan const continue defer do else enum effect
false fn for if impl import in let loop match mod move mut package proof pub
recv ref return select send spawn static struct trait true type unsafe use
where while yield
```

Contextual keywords:

```text
auto cap closed erased ensures forall ghost given implicit impossible invariant
linear partial post pre requires rewrite shared terminates total unique with
```

## 4. Modules And Packages

Avtan modules map to Go packages.

```avtan
package app.worker

import std.time
import std.sync.{TaskGroup, Chan}
import github.com.acme.store as store
```

Rules:

1. A package contains one or more `.avtn` files in one directory.
2. `pub` items are exported from the package.
3. Non-`pub` items are package-private.
4. A package may define an `init` function with signature `fn init()`.
5. There is no global mutable state unless declared `static mut`, which requires
   an effect annotation.

Go lowering:

1. Package paths map to Go import paths through the project manifest.
2. Exported Avtan identifiers are converted to exported Go identifiers.
3. Multiple Avtan source files in one package generate one Go package directory.

## 5. Values And Types

### 5.1 Primitive Types

```text
bool
i8 i16 i32 i64 int
u8 u16 u32 u64 uint
isize usize
f32 f64
char
str
unit
never
```

`str` is immutable UTF-8 text and lowers to Go `string`.

`unit` has one value, `()`, and lowers to an empty Go result position or a
zero-sized generated type when a value is required.

`never` is the type of non-returning expressions such as `panic`.

### 5.2 Numeric Rules

Numeric literals are untyped until constrained.

Avtan does not allow implicit lossy numeric conversion.

```avtan
let x: i32 = 10
let y: i64 = i64(x)
```

Compile-time integer expressions may appear in dependent type parameters.

### 5.3 Tuple Types

```avtan
let pair: (str, i32) = ("port", 8080)
```

Tuples lower to generated Go structs unless optimized away by the backend.

### 5.4 Arrays And Slices

```avtan
let fixed: [i32; 4] = [1, 2, 3, 4]
let view: []i32 = fixed.as_slice()
```

Fixed arrays may carry length as an ordinary value-level natural that appears in
the type:

```avtan
fn first<const N: Nat>(xs: [i32; S(N)]) -> i32
{
    xs[0]
}
```

Go lowering:

1. `[T; n]` lowers to `[n]T` when `n` is statically known after elaboration.
2. `[]T` lowers to `[]T`.
3. Dependent slice lengths lower to wrapper structs when the length must be
   preserved dynamically.

### 5.5 Structs

```avtan
pub struct User {
    pub id: UserId,
    name: str,
    active: bool,
}
```

Struct update:

```avtan
let next = User { active: true, ..user }
```

Go lowering:

1. Plain structs lower to Go structs.
2. Private fields lower to unexported fields.
3. Refined fields may generate constructors and validation methods.

### 5.6 Enums

Avtan enums are algebraic data types.

```avtan
pub enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

Pattern matching must be exhaustive.

Go lowering options:

1. Interface plus variant structs for payload-carrying enums.
2. Integer constants for fieldless enums.
3. Tagged struct representation when configured for performance.

The representation is stable only if marked:

```avtan
#[repr(tagged)]
enum Message {
    Ping,
    Data([]u8),
}
```

### 5.7 Type Aliases And Newtypes

```avtan
type UserId = u64

pub struct Email(str)
where
    self.0.contains("@")
```

Aliases do not create distinct types. Tuple structs do.

## 6. Functions

```avtan
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

Expression body:

```avtan
fn square(x: i64) -> i64 = x * x
```

Named return values are not part of Avtan source syntax, even though the Go
backend may generate them.

### 6.1 Pre And Postconditions

```avtan
fn divide(a: i64, b: i64) -> i64
requires b != 0
ensures result * b == a
{
    a / b
}
```

`requires` is checked at call sites when possible. If a call-site proof depends
on runtime values, the compiler inserts a generated runtime check unless the
function is marked `#[proof_only]`.

`ensures` becomes:

1. A static proof obligation for callers that use the postcondition.
2. A debug assertion in generated Go when debug contracts are enabled.

### 6.2 Generic Functions

```avtan
fn map<T, U>(xs: []T, f: fn(T) -> U) -> []U {
    let out = Vec<U>::with_cap(xs.len())
    for x in xs {
        out.push(f(x))
    }
    out.into_slice()
}
```

Generic functions lower to Go generics when possible. If a dependent parameter is
erased, it does not appear in generated Go. If a runtime-relevant dependent
parameter cannot be represented by Go generics, the compiler monomorphizes or
emits a wrapper.

### 6.3 Dependent Parameters

Avtan supports ordinary parameters, type generics, dependent value generics, and
proof-only parameters. The preferred surface syntax follows Rust generics.

```avtan
fn id<T>(x: T) -> T = x

fn head<T, const N: Nat>(xs: Vect<T, S(N)>) -> T

fn chunk<const N: Nat>(bytes: Vect<u8, N>, size: Nat)
    -> exists<const Count: Nat> Vect<Vect<u8, size>, Count>
requires size > 0
```

Parameter forms:

1. `(x: A)`: explicit runtime or compile-time parameter.
2. `<T>`: type parameter, elaborated as an implicit type argument.
3. `<const N: Nat>`: dependent value parameter, usually inferred and erased.
4. `{p: P}` or `{implicit p: P}`: implicit parameter inserted by elaboration.
5. `{auto p: P}`: implicit proof found by proof search.
6. `{erased p: P}`: compile-time-only proof argument removed before Go lowering.

The core type of a dependent function is a Pi type:

```avtan
(x: A) -> B(x)
{implicit x: A} -> B(x)
{auto p: P(x)} -> B(x)
{erased p: P(x)} -> B(x)
```

## 7. Expressions

### 7.1 Blocks

Blocks are expressions. The last expression is the block value.

```avtan
let x = {
    let y = read()
    y + 1
}
```

### 7.2 If

```avtan
let mode = if debug { "debug" } else { "release" }
```

Both branches must unify to a common type.

### 7.3 Match

```avtan
match result {
    Ok(value) => value,
    Err(err) => return Err(err),
}
```

Match is exhaustive. Guards are supported:

```avtan
match n {
    x if x < 0 => "negative",
    0 => "zero",
    _ => "positive",
}
```

### 7.4 Loops

```avtan
for item in items {
    process(item)
}

while socket.open() {
    socket.poll()
}

loop {
    break
}
```

Loops that participate in proofs may require invariants:

```avtan
while i < xs.len()
invariant i <= xs.len()
{
    i += 1
}
```

## 8. Ownership And Borrowing

Avtan uses ownership to prevent data races and invalid aliasing in source code.
The runtime is still Go's runtime.

### 8.1 Ownership Modes

Every value is in one of three modes:

1. `unique T`: one owner, mutable access allowed.
2. `shared T`: multiple readers, no mutation through shared references.
3. `linear T`: must be consumed exactly once.

Plain `T` means `unique T` unless `T` is copy.

### 8.2 Copy And Move

Primitive numbers, `bool`, `char`, and explicitly declared `Copy` types are
copied by assignment. Other values move by default.

```avtan
let a = File::open(path)?
let b = a
// a is unavailable here
```

### 8.3 Borrowing

```avtan
fn len(xs: &[]u8) -> usize
fn fill(xs: &mut []u8, value: u8)
```

Rules:

1. Any number of immutable borrows may exist at once.
2. A mutable borrow is exclusive.
3. Borrows cannot escape their owner unless the lifetime is explicit and valid.
4. Values sent to another task must be `Send`.
5. Shared values accessed by multiple tasks must be `Sync`.

Avtan lifetimes are inferred by default. Explicit lifetimes are available for
library boundaries:

```avtan
fn get<'a>(map: &'a Map<str, Value>, key: &str) -> Option<&'a Value>
```

Go lowering:

1. Borrowing is a source-level static discipline.
2. The Go output uses pointers, slices, or values according to escape analysis.
3. The compiler may copy values when needed to preserve source aliasing rules.

## 9. Error Handling

Fallible functions return `Result<T, E>`.

```avtan
fn load(path: Path) -> Result<Config, IoError> {
    let bytes = fs::read(path)?
    Config::parse(bytes)
}
```

The `?` operator propagates `Err`.

Go lowering:

```go
value, err := fs.Read(path)
if err != nil {
    return zero, err
}
```

Avtan does not have exceptions.

## 10. Traits And Implementations

Traits describe behavior.

```avtan
pub trait Display {
    fn fmt(self: &Self) -> str
}

impl Display for User {
    fn fmt(self: &Self) -> str {
        self.name
    }
}
```

Trait bounds:

```avtan
fn print<T: Display>(value: &T) {
    log(value.fmt())
}
```

Go lowering:

1. Object-safe traits lower to Go interfaces.
2. Generic trait bounds lower to Go type constraints where possible.
3. Non-object-safe traits lower through dictionaries or monomorphization.

### 10.1 Built-In Marker Traits

```text
Copy
Clone
Drop
Send
Sync
Sized
Zero
Eq
Ord
Hash
```

`Send` means a value may be moved to another task.

`Sync` means shared references to a value may be used concurrently.

## 11. Full Dependent Types

Avtan has full Idris-style dependent types. Values may appear in types, functions
may return types that mention their arguments, and proofs are ordinary total
programs whose results are erased before Go generation when they have no runtime
content.

The surface language is Rust-like, but the semantic core is a small dependently
typed lambda calculus with algebraic data, universes, Pi types, Sigma types,
equality, implicit arguments, holes, and erasure annotations.

### 11.1 Universes

Types live in a cumulative hierarchy:

```avtan
Type 0 : Type 1
Type 1 : Type 2
Type n : Type (n + 1)
```

`Type` without an explicit level means an inferred universe level. Avtan does not
allow `Type : Type`.

### 11.2 Pi Types And Dependent Functions

A function type may bind a value and use it in the return type.

```avtan
(x: A) -> B(x)
```

Rust-like function syntax elaborates to Pi types:

```avtan
fn id<T>(x: T) -> T = x

fn head<T, const N: Nat>(xs: Vect<T, S(N)>) -> T
```

The parameter `const N: Nat` is a dependent value parameter. It is normally
erased and inferred by elaboration, so callers write `head(xs)` rather than
passing `N` manually. This looks like Rust const generics, but Avtan allows these
parameters to range over user-defined total types, not only machine constants.

### 11.3 Sigma Types And Dependent Pairs

Sigma types carry a value together with another value whose type depends on the
first value.

Core notation is `(x: A ** B(x))`. The preferred surface spelling is:

```avtan
exists<const N: Nat> Vect<T, N>
```

Example:

```avtan
fn read_vec<T>(path: Path) -> Result<exists<const N: Nat> Vect<T, N>, IoError>
```

Go lowering keeps only runtime-relevant fields. Erased proof fields are removed.

### 11.4 Dependent Enums And Structs

Dependent type families are written with Rust-like `enum` and `struct`
declarations. A generic parameter introduced with `const` is a value-level type
index.

```avtan
enum Nat {
    Z,
    S(Nat),
}

enum Vect<T, const N: Nat> {
    Nil
        where N == Z,

    Cons<const M: Nat> {
        head: T,
        tail: Vect<T, M>,
    }
        where N == S(M),
}
```

Internally, `Vect<T, const N: Nat>` elaborates to a type family whose core shape
is equivalent to `Type -> Nat -> Type`. That arrow form is compiler/type-theory
semantics, not the preferred surface syntax. Users should normally read the
declaration as: "`Vect` is a vector type indexed by its element type and length."

Variant `where` clauses refine the enum index. `Nil` is available only when
`N == Z`, while `Cons` is available only when `N == S(M)`. This keeps the surface
syntax close to Rust while elaborating to the same dependent core as an
Idris-style indexed data family.

Functions use the same Rust-like generic syntax:

```avtan
fn append<T, const M: Nat, const N: Nat>(
    left: Vect<T, M>,
    right: Vect<T, N>,
) -> Vect<T, M + N>
```

### 11.5 Equality, Refl, And Rewrite

Propositional equality is an ordinary type:

```avtan
x == y
```

`Refl` proves equality when both sides normalize to the same core term.

```avtan
proof fn plus_zero_right(n: Nat) -> n + Z == n {
    match n {
        Z => Refl,
        S(k) => rewrite plus_zero_right(k) in Refl,
    }
}
```

`rewrite proof in expr` rewrites the expected type of `expr` using an equality
proof.

### 11.6 Dependent Pattern Matching

Pattern matching refines the types of branch bodies.

```avtan
fn head<T, const N: Nat>(xs: Vect<T, S(N)>) -> T {
    match xs {
        Vect::Cons { head, .. } => head,
    }
}
```

Impossible branches are allowed when the context is contradictory:

```avtan
fn absurd_head<T>(xs: Vect<T, Z>) -> T {
    match xs {
        Nil => impossible,
    }
}
```

The compiler performs coverage checking for all total definitions.

### 11.7 Totality And Termination

Definitions used in types, proofs, erased arguments, or compile-time computation
must be total.

```avtan
total fn plus(a: Nat, b: Nat) -> Nat {
    match a {
        Z => b,
        S(k) => S(plus(k, b)),
    }
}
```

Runtime-only general recursion is allowed only in `partial fn` definitions.
`partial fn` results cannot be used by the type checker as compile-time values.

The totality checker must support at least:

1. Structural recursion.
2. Lexicographic recursion.
3. Mutual recursion with size-change analysis.
4. Coverage checking for pattern matches.

### 11.8 Implicit Arguments, Auto Search, And Holes

The elaborator inserts implicit arguments, solves metavariables, and reports
unsolved holes.

```avtan
fn map<A, B, const N: Nat>(f: fn(A) -> B, xs: Vect<A, N>) -> Vect<B, N>

let ys = map(double, xs)
```

Auto implicits request proof search:

```avtan
fn safe_index<T, const N: Nat>(
    xs: Vect<T, N>,
    i: Fin<N>,
    {auto p: InBounds<i, N>},
) -> T
```

Typed holes are allowed during development:

```avtan
let proof = ?missing_proof
```

The compiler reports each hole with its context and expected type.

### 11.9 Erasure

Arguments and fields that exist only for type checking are erased before Go
lowering.

```avtan
fn length<T, const N: Nat>(xs: Vect<T, N>) -> Nat
```

Erasure rules:

1. Types, proofs, implicit-only evidence, and erased parameters do not lower to
   Go.
2. Runtime-relevant indices lower to Go fields or arguments.
3. Erased values cannot affect runtime control flow.
4. A value may be erased only if the erasure checker proves it is irrelevant at
   runtime.

### 11.10 Refinements As Dependent Types

Refinement types are syntactic sugar over dependent pairs plus a proof.

```avtan
type Port = (value: u16 ** value > 0 && value <= 65535)
```

The ergonomic spelling is still allowed:

```avtan
type Port = u16 where self > 0 && self <= 65535
```

Construction from runtime data requires a decision procedure:

```avtan
fn parse_port(raw: u16) -> Result<Port, RefinementError> {
    decide raw > 0 && raw <= 65535
}
```

Unchecked construction requires `unsafe proof`.

### 11.11 Contracts And Runtime Decisions

Full dependent types do not mean the compiler silently proves arbitrary runtime
facts. If a proof is required, it must be produced by elaboration, user code,
normalization, auto search, or an explicit decision procedure.

`requires` and `ensures` are contract syntax over dependent propositions:

```avtan
fn get<T>(xs: []T, i: usize) -> T
requires i < xs.len()
```

When a caller has no proof, the compiler may insert a runtime decision only for
contracts declared decidable. Otherwise the program is rejected until the caller
passes or constructs evidence.

### 11.12 Go Lowering

Only the erased runtime fragment lowers to Go.

Go lowering must happen after:

1. Elaboration.
2. Type checking.
3. Totality and coverage checks.
4. Ownership and effect checks.
5. Erasure.

If a dependent value is runtime-relevant, it is represented explicitly in Go. If
it is type-only evidence, it is removed.

## 12. Effects

Effects make side effects visible in function signatures.

```avtan
effect IO
effect Net
effect Clock
effect Spawn
effect Unsafe
```

Function effects:

```avtan
fn read_config(path: Path) -> Result<Config, IoError> effects(IO)
```

Effects are inferred inside a package, but public functions must have explicit
effect annotations unless configured otherwise.

Effects compose:

```avtan
fn serve(addr: Addr) -> Result<(), ServerError> effects(Net, Spawn, Clock)
```

Pure functions have no effects and may be used in proofs if they terminate.

## 13. Concurrency Model

Avtan concurrency is structured by default. A detached task is possible, but it
must be explicit.

### 13.1 Tasks

```avtan
let task = spawn compute(input)
let value = task.await?
```

`spawn` starts a child task. The spawned expression must:

1. Have the `Spawn` effect.
2. Capture only `Send` values by move or `Sync` shared references.
3. Return `T` or `Result<T, E>`.

Go lowering:

1. `spawn expr` lowers to `go func() { ... }()`.
2. The result is delivered through a generated one-shot channel.
3. Panic propagation is configurable. The default converts panic to task failure.

### 13.2 Task Groups

Task groups provide structured concurrency.

```avtan
let group = TaskGroup::new(ctx)

let a = group.spawn fetch_user(id)
let b = group.spawn fetch_orders(id)

let user = a.await?
let orders = b.await?

group.join()?
```

Rules:

1. A task spawned in a group must complete before the group is dropped.
2. If one task fails, the default policy cancels sibling tasks.
3. Group cancellation is represented by a typed `CancelToken`.

Go lowering uses `context.Context`, `sync.WaitGroup`, and generated result
channels.

### 13.3 Channels

```avtan
let (tx, rx) = chan<UserEvent>(capacity = 128)

tx.send(event).await?
let event = rx.recv().await?
```

Channel endpoint types:

```avtan
SendChan<T>
RecvChan<T>
Chan<T>
```

Capabilities:

```avtan
fn producer(tx: SendChan<Message>) effects(IO)
fn consumer(rx: RecvChan<Message>) effects(IO)
```

Go lowering:

1. `Chan<T>` lowers to `chan T`.
2. `SendChan<T>` lowers to `chan<- T`.
3. `RecvChan<T>` lowers to `<-chan T`.

### 13.4 Select

```avtan
select {
    msg = rx.recv() => handle(msg),
    _ = clock.after(1.sec()) => timeout(),
    _ = ctx.done() => return Err(Cancelled),
}
```

`select` is an expression if all branches produce a common type.

The compiler checks that channel operations in `select` are non-blocking at the
source level and lower directly to Go `select`.

### 13.5 Mutex And Shared State

```avtan
let cache = Mutex::new(Map<Key, Value>::new())

cache.lock(|state| {
    state.insert(key, value)
})
```

The closure receives `&mut T`. The guard cannot escape the closure unless using
the lower-level guard API:

```avtan
let guard = cache.lock()
guard.insert(key, value)
drop(guard)
```

Go lowering uses `sync.Mutex` or `sync.RWMutex`.

### 13.6 Atomics

```avtan
let counter = Atomic<u64>::new(0)
counter.fetch_add(1, Ordering::Relaxed)
```

Supported orderings:

```text
Relaxed
Acquire
Release
AcqRel
SeqCst
```

Go lowering uses `sync/atomic`.

### 13.7 Linear Resources

Linear types help model handles and protocols.

```avtan
linear struct Tx {
    conn: DbConn,
}

impl Tx {
    fn commit(self) -> Result<(), DbError>
    fn rollback(self) -> Result<(), DbError>
}
```

A linear value must be consumed exactly once on every control-flow path.

### 13.8 Session-Typed Channels

The dependent type system can express protocol state.

```avtan
enum Handshake {
    Start,
    Authed,
    Closed,
}

struct Conn(state: Handshake) {
    raw: TcpConn,
}

fn auth(conn: Conn<Handshake::Start>, token: Token)
    -> Result<Conn<Handshake::Authed>, AuthError>

fn close(state: Handshake, conn: Conn<state>) -> Conn<Handshake::Closed>
```

This is the preferred pattern for compile-time protocol safety.

## 14. Async

Avtan has `async` syntax for source clarity, but the Go backend lowers async
operations to blocking goroutine/channel code unless a package opts into a
specific runtime adapter.

```avtan
async fn fetch(url: Url) -> Result<Response, NetError> effects(Net)

let response = fetch(url).await?
```

Rules:

1. `await` is allowed only in `async fn`, task bodies, and select branches.
2. Async functions are cancellable if they accept or capture a `CancelToken`.
3. Async functions must not hold a mutable borrow across an `await` unless the
   borrowed value is pinned to the same task.

Go lowering:

1. Plain `async fn` may lower to a function returning `Task<T>`.
2. Direct calls may be optimized to blocking calls when no concurrency boundary
   is needed.

## 15. Cancellation And Time

Cancellation is explicit.

```avtan
fn work(ctx: CancelToken) -> Result<(), Cancelled> effects(IO) {
    while !ctx.cancelled() {
        step()?
    }
    Ok(())
}
```

Standard time primitives:

```avtan
clock.now()
clock.sleep(duration).await
clock.after(duration)
```

Go lowering uses `context.Context` and `time`.

## 16. Memory Safety And Unsafe

`unsafe` is allowed but isolated.

```avtan
unsafe fn from_raw(ptr: RawPtr<u8>, len: usize) -> []u8
effects(Unsafe)
```

Unsafe blocks:

```avtan
let bytes = unsafe {
    from_raw(ptr, len)
}
```

Unsafe code may:

1. Call unsafe functions.
2. Use raw pointers.
3. Use unchecked refinement casts.
4. Request backend `unsafe` emission.

Unsafe code must still be syntactically scoped and effect-annotated.

## 17. Standard Library Surface

The standard library is intentionally small in the language spec. Packages may
grow over time.

Required packages:

```text
std.core
std.nat
std.vect
std.fin
std.equality
std.dec
std.result
std.option
std.vec
std.map
std.set
std.io
std.fs
std.net
std.time
std.sync
std.atomic
std.proof
```

Required core types:

```text
Type
Nat
Fin<n>
Vect<T, n>
Dec<P>
x == y
Refl
Option<T>
Result<T, E>
Vec<T>
Map<K, V>
Set<T>
Task<T, E>
TaskGroup<E>
CancelToken
Chan<T>
SendChan<T>
RecvChan<T>
Mutex<T>
RwMutex<T>
Atomic<T>
Proof<P>
```

## 18. Go Interoperability

Avtan can import Go packages through explicit bindings.

```avtan
import go "net/http" as http

extern go {
    fn http.Get(url: str) -> Result<http.Response, http.Error> effects(Net)
}
```

Rules:

1. Go functions returning `(T, error)` map to `Result<T, error>`.
2. Go functions returning only `error` map to `Result<(), error>`.
3. Go interfaces may map to Avtan traits when method sets are compatible.
4. Go structs may be imported as opaque or transparent.
5. Go panics are not part of normal Avtan error handling.

External declarations must state ownership and concurrency traits for imported
types:

```avtan
extern go type http.Client: Send + Sync
```

## 19. Attributes

Attributes attach metadata to items.

```avtan
#[go(name = "ServeHTTP")]
#[derive(Clone, Eq)]
pub struct Handler { ... }
```

Required attributes:

```text
#[derive(...)]
#[go(name = "...")]
#[go(package = "...")]
#[proof_only]
#[repr(...)]
#[test]
#[bench]
#[cfg(...)]
#[allow(...)]
#[deny(...)]
```

## 20. Testing

Tests are ordinary functions marked `#[test]`.

```avtan
#[test]
fn validates_port() {
    let port: Port = 8080
    assert(port == 8080)
}
```

Proof tests are compile-time tests:

```avtan
#[test]
proof fn push_increases_len() {
    let xs: Vect<i32, 3> = vect![1, 2, 3]
    let ys = xs.push(4)
    prove ys.len() == 4
}
```

Generated Go tests use the standard `testing` package.

## 21. Manifest

An Avtan project has `avtan.toml`.

```toml
[package]
name = "example"
version = "0.1.0"

[backend.go]
module = "github.com/acme/example"
target = "1.22"
contracts = "debug"

[elaboration]
max_holes = 256
auto_search_depth = 8

[totality]
required = true
max_reduction_steps = 100000

[contracts]
runtime_decisions = true
```

The Rust implementation of the compiler may still use `Cargo.toml`; `avtan.toml`
describes Avtan packages being compiled.

## 22. Example Program

```avtan
package example

import std.net
import std.sync.{TaskGroup, Chan}
import std.time

type Port = u16 where self > 0

struct Request {
    id: u64,
    body: str,
}

struct Response {
    request_id: u64,
    status: u16 where self >= 100 && self < 600,
}

fn handle(req: Request) -> Result<Response, Error> effects(IO) {
    Ok(Response {
        request_id: req.id,
        status: 200,
    })
}

fn serve(port: Port, rx: RecvChan<Request>, tx: SendChan<Response>)
    -> Result<(), Error>
    effects(IO, Spawn)
{
    let ctx = CancelToken::background()
    let group = TaskGroup::new(ctx)

    loop {
        select {
            req = rx.recv() => {
                let tx2 = tx.clone()
                group.spawn(move {
                    let response = handle(req)?
                    tx2.send(response).await?;
                    Ok(())
                })
            },
            _ = time.after(30.sec()) => break,
        }
    }

    group.join()
}
```

## 23. Initial MVP

The first implementation should be a vertical slice of the full dependent type
architecture, not a separate simply typed language.

Required in the first dependent-core MVP:

1. Lexer, parser, formatter, and AST.
2. Packages, imports, functions, structs, simple enums, and dependent
   `struct`/`enum` declarations.
3. Core terms for universes, variables, lambdas, Pi types, applications, lets,
   constructors, case trees, holes, and erased arguments.
4. Elaboration from surface syntax into core with implicit arguments and
   metavariables.
5. Normalization by evaluation or another explicit normalization strategy.
6. Definitional equality and unification for elaboration.
7. Built-in `Nat`, propositional equality, `Refl`, and `rewrite`.
8. Dependent `Vect<A, n>` example compiling and type-checking.
9. Coverage and structural termination checks for total functions.
10. Erasure of proofs, implicit-only terms, and compile-time indices.
11. Go code generation for the erased runtime fragment.
12. Test generation to Go `testing` plus compile-time proof tests.

Features explicitly not required in the first dependent-core MVP:

1. Full trait objects.
2. Advanced proof automation or tactics.
3. Coinduction and codata.
4. General partial functions inside types.
5. Backend `unsafe`.
6. Custom async runtimes.
7. Optimized Go representation for every dependent enum/struct encoding.

## 24. Open Design Questions

1. Should Avtan use significant semicolons or keep Rust-like optional expression
   semicolons?
2. Should public function effects be mandatory from version 0.1, or introduced
   after inference stabilizes?
3. Should generated Go favor readability or fewer allocations when enum payloads
   are involved?
4. How much Go interop should be automatic versus generated through explicit
   binding files?
5. Should optional proof automation be built into the compiler, delegated to an
   external solver, or support both?
6. Should `async` be part of the MVP syntax if the first backend uses blocking
   goroutines?
