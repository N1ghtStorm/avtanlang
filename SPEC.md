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
3. Support a practical subset of dependent typing without requiring a full
   theorem prover in the compiler.
4. Use ownership and borrowing for local safety, but map predictably to Go's
   garbage-collected runtime.
5. Preserve source-level invariants in generated Go through a mix of compile-time
   proofs, generated runtime assertions, and generated wrapper types.
6. Prefer simple backend rules over clever optimizations in the first versions.

Avtan is not trying to be:

1. A drop-in Rust replacement.
2. A proof assistant.
3. A Go syntax variant.
4. A language that exposes unsafe pointer arithmetic as a normal programming
   model.

## 2. Compilation Model

The compiler pipeline is:

1. Parse Avtan source into an AST.
2. Resolve names, modules, imports, and type aliases.
3. Elaborate dependent types and refinement predicates into an internal core.
4. Type-check ordinary types, ownership, effects, and concurrency capabilities.
5. Discharge compile-time proofs using normalization and a bounded SMT-like
   solver.
6. Insert runtime checks where the spec allows unresolved dynamic obligations.
7. Lower Avtan core to Go AST.
8. Format and emit Go modules.

The generated Go is part of the public contract. A user should be able to inspect
and debug it. Generated names may be mangled, but must be deterministic.

### 2.1 Target Go

The default backend target is Go 1.22 or newer. The compiler may support older
or newer target profiles through configuration.

Generated Go must avoid reflection unless a feature explicitly requires it.
Generated Go must not use `unsafe` unless an Avtan package opts into an `unsafe`
backend feature.

### 2.2 Source Files

Avtan source files use the extension `.avt`.

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
cap closed ensures forall ghost given invariant linear post pre requires
shared terminates unique
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

1. A package contains one or more `.avt` files in one directory.
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

Fixed arrays may carry length as a type-level natural:

```avtan
fn first<const N: Nat>(xs: [i32; N]) -> i32
where
    N > 0
{
    xs[0]
}
```

Go lowering:

1. `[T; N]` lowers to `[N]T` when `N` is statically known.
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

Generic functions lower to Go generics when possible. If a dependent parameter
cannot be represented by Go generics, the compiler monomorphizes or emits a
wrapper.

### 6.3 Const Parameters

```avtan
fn chunk<const N: Nat>(bytes: []u8) -> Vec<[u8; N]>
where
    N > 0
```

Const parameters are available to type-level expressions. MVP const parameters
support:

1. `Nat`
2. `Int`
3. `Bool`
4. fieldless enums
5. string literals for labels and protocol states

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

## 11. Dependent And Refinement Types

Avtan supports dependent types in a deliberately restricted fragment.

The compiler must always terminate while type-checking ordinary programs. It may
reject true statements that are outside the supported proof fragment.

### 11.1 Refinement Types

A refinement type restricts values with a predicate. It has the same runtime
representation as the base type, but a distinct static type.

```avtan
type NonZeroI64 = i64 where self != 0
type Port = u16 where self > 0 && self <= 65535
```

Construction:

```avtan
let port: Port = Port::new(8080)?
```

For literal or proven values:

```avtan
let port: Port = 8080
```

Runtime construction returns `Result<Port, RefinementError>` unless the caller
uses an explicit checked assertion:

```avtan
let port = assume<Port>(raw)
```

`assume` requires `unsafe proof`.

### 11.2 Indexed Types

Types may be indexed by compile-time values.

```avtan
struct Vec<T, const N: Nat> {
    data: []T,
}
where
    data.len() == N
```

Examples:

```avtan
fn push<T, const N: Nat>(xs: Vec<T, N>, x: T) -> Vec<T, N + 1>

fn append<T, const A: Nat, const B: Nat>(
    left: Vec<T, A>,
    right: Vec<T, B>,
) -> Vec<T, A + B>
```

### 11.3 Value-Dependent Function Types

Function return types may reference arguments.

```avtan
fn take<T>(xs: []T, n: usize) -> Slice<T>
requires n <= xs.len()
ensures result.len() == n
```

MVP syntax keeps value dependencies in `requires` and `ensures` rather than
allowing arbitrary expression syntax inside every type.

### 11.4 Proof Values

Proof values exist only at compile time and do not lower to Go.

```avtan
proof fn len_append<T, const A: Nat, const B: Nat>(
    left: Vec<T, A>,
    right: Vec<T, B>,
) -> Proof<append(left, right).len() == A + B>
```

Rules:

1. `proof fn` cannot perform IO.
2. `proof fn` cannot spawn tasks.
3. `proof fn` cannot inspect runtime-only values.
4. `proof fn` must terminate.

### 11.5 Ghost Values

Ghost values support proofs without runtime representation.

```avtan
ghost let original_len = xs.len()
xs.push(item)
prove xs.len() == original_len + 1
```

Ghost values cannot affect runtime control flow.

### 11.6 Solver Fragment

The required solver fragment includes:

1. Boolean logic.
2. Linear integer arithmetic.
3. Natural number comparisons.
4. Equality over uninterpreted symbols.
5. Algebraic datatype constructor equality.
6. Finite enum reasoning.
7. Length reasoning for arrays, slices, strings, and vectors.

The compiler may support additional theories, but portable packages must not
depend on them unless declared in the manifest.

### 11.7 Dynamic Obligations

When a proof depends on runtime data, the compiler classifies the obligation:

1. `static`: proven at compile time.
2. `dynamic`: checked at runtime.
3. `rejected`: cannot be checked soundly or cheaply enough.

Example:

```avtan
fn get(xs: []i32, i: usize) -> i32
requires i < xs.len()
{
    xs[i]
}

fn caller(xs: []i32, i: usize) -> i32 {
    get(xs, i) // inserts runtime check unless caller proves the precondition
}
```

Generated Go must preserve runtime checks unless compiled with an explicit
unchecked profile.

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

struct Conn<const S: Handshake> {
    raw: TcpConn,
}

fn auth(conn: Conn<Handshake::Start>, token: Token)
    -> Result<Conn<Handshake::Authed>, AuthError>

fn close<const S: Handshake>(conn: Conn<S>) -> Conn<Handshake::Closed>
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
    let xs: Vec<i32, 3> = vec![1, 2, 3]
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

[solver]
runtime_checks = true
max_steps = 100000
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

The first implementation should target a smaller useful subset:

1. Lexer, parser, formatter, and AST.
2. Packages, imports, functions, structs, fieldless enums, and simple
   payload-carrying enums.
3. Primitive types, arrays, slices, tuples, and `Result`.
4. Basic generics and const `Nat` parameters.
5. Refinement aliases with literal proofs and generated runtime constructors.
6. `requires` checks at call sites.
7. Exhaustive `match`.
8. Source-level ownership moves and simple immutable/mutable borrowing.
9. `spawn`, `Task`, `Chan`, `select`, and `TaskGroup`.
10. Go code generation for the subset above.
11. Go interop for simple functions and `(T, error)` results.
12. Test generation to Go `testing`.

Features explicitly not required in MVP:

1. Full trait objects.
2. Higher-rank lifetimes.
3. General recursive proof functions.
4. Session-typed channels beyond const enum state parameters.
5. Backend `unsafe`.
6. Custom async runtimes.

## 24. Open Design Questions

1. Should Avtan use significant semicolons or keep Rust-like optional expression
   semicolons?
2. Should public function effects be mandatory from version 0.1, or introduced
   after inference stabilizes?
3. Should generated Go favor readability or fewer allocations when enum payloads
   are involved?
4. How much Go interop should be automatic versus generated through explicit
   binding files?
5. Should dependency solving be embedded in the compiler, delegated to an
   external solver, or support both?
6. Should `async` be part of the MVP syntax if the first backend uses blocking
   goroutines?
