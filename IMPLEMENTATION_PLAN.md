# План реализации Avtan

Status: draft 0.1

Этот файл описывает практический порядок реализации языка Avtan из `SPEC.md`.
Главный принцип: с самого начала строить компилятор вокруг полного
Idris-подобного dependent core, а не добавлять зависимые типы потом как
надстройку. Первый результат должен быть маленьким, но он уже обязан проходить
через elaboration, normalization, dependent type checking, totality checking,
erasure и только затем Go lowering.

## 1. Целевой MVP

Первый пригодный результат:

1. CLI-команда `avtan build`.
2. Чтение `avtan.toml`.
3. Парсинг одного пакета из `.avtn` файлов.
4. Диагностики с позициями в исходнике.
5. AST для функций, Rust-like `struct`/`enum`, выражений, `match`, dependent
   generic params, holes, equality proofs и `rewrite`.
6. Dependent core с universes, Pi, Sigma, lambdas, applications, constructors,
   case trees и erased binders.
7. Elaboration из surface syntax в core.
8. Normalization и definitional equality.
9. Type checking для зависимых функций и dependent `struct`/`enum`.
10. Coverage и structural termination для total definitions.
11. Erasure proof/type-only значений.
12. Генерация читаемого Go-кода из erased runtime core.
13. Компиляция примера `Nat`, `Vect`, `head`, `append`.

Все остальные фичи считаются post-MVP, пока этот dependent-core slice не работает
end-to-end.

## 2. Архитектура Компилятора

Рекомендуемая структура Rust-модулей:

```text
src/
  main.rs
  cli.rs
  source/
    mod.rs
    file.rs
    span.rs
    interner.rs
  diagnostics/
    mod.rs
    error.rs
    report.rs
  lexer/
    mod.rs
    token.rs
  parser/
    mod.rs
    ast.rs
  hir/
    mod.rs
    lower.rs
  core/
    mod.rs
    term.rs
    ty.rs
    env.rs
    levels.rs
  elab/
    mod.rs
    holes.rs
    implicit.rs
    check.rs
  nbe/
    mod.rs
    value.rs
    quote.rs
  unify/
    mod.rs
  resolve/
    mod.rs
  types/
    mod.rs
    surface.rs
  proof/
    mod.rs
    equality.rs
    search.rs
  coverage/
    mod.rs
  totality/
    mod.rs
  erase/
    mod.rs
    ir.rs
  ownership/
    mod.rs
  effects/
    mod.rs
  go/
    mod.rs
    ast.rs
    emit.rs
    lower.rs
  project/
    mod.rs
    manifest.rs
  tests/
```

Разделение по слоям:

1. `AST`: максимально близко к синтаксису.
2. `HIR`: нормализованное дерево после name resolution.
3. `Core`: маленькое зависимо-типизированное ядро с universes, Pi, Sigma,
   lambdas, applications, constructors, case trees, holes и erased binders.
4. `Elaboration`: implicit arguments, metavariables, holes, surface-to-core
   translation.
5. `NBE/DefEq`: normalization by evaluation и definitional equality.
6. `Coverage/Totality`: проверки, которые делают proof/type-level computation
   sound.
7. `Erased IR`: runtime-only программа после удаления proofs и type-only terms.
8. `Go AST`: backend-представление, которое затем печатается в `.go`.

## 3. Этап 0: Репозиторный Каркас

Цель: подготовить проект так, чтобы остальные этапы можно было проверять
автоматически.

Задачи:

1. Добавить CLI на `clap` или минимальный ручной парсер аргументов.
2. Добавить команды:
   1. `avtan check`
   2. `avtan build`
   3. `avtan fmt`
   4. `avtan test`
3. Добавить систему исходников: file id, spans, line/column mapping.
4. Добавить диагностики с кодом ошибки, сообщением, span и подсказкой.
5. Добавить snapshot-тесты для диагностик.

Definition of done:

1. `cargo test` проходит.
2. Ошибка парсинга показывает файл, строку и колонку.
3. CLI умеет прочитать файл и вывести токены в debug-режиме.

## 4. Этап 1: Lexer

Цель: надежно разбивать `.avtn` файлы на токены.

Задачи:

1. Реализовать токены:
   1. identifiers
   2. keywords
   3. integer, float, string, char literals
   4. comments
   5. operators
   6. delimiters
   7. attributes `#[...]`
2. Сохранять spans для всех токенов.
3. Поддержать doc comments как отдельные токены или trivia.
4. Реализовать восстановление после неизвестного символа.
5. Добавить golden-тесты для токенизации.

Definition of done:

1. Lexer не падает на произвольном тексте.
2. Все ключевые слова из `SPEC.md` распознаются.
3. Комментарии не ломают spans последующих токенов.

## 5. Этап 2: Parser И AST

Цель: получить полное синтаксическое дерево для первого dependent-core slice.

Текущее состояние:

1. Сделано: parser покрывает package/imports, attributes, Rust-like `struct` и
   `enum`, dependent generics `const N: Nat`, variant `where`, `fn`/`proof fn`,
   blocks, `let`, `if`, `match`, `for`, `while`, `loop`, `return`, `break`,
   `continue`, calls, field access, indexing, holes, `rewrite`, `impossible`,
   `requires`, `ensures`, explicit, implicit, auto и erased binders.
2. Сделано: добавлен стабильный AST dump для CLI и parser fixture-тестов.
3. Осталось: более сильный error recovery и расширение snapshot-набора на
   negative recovery-кейсы.

Задачи:

1. Реализовать parser для:
   1. package declarations
   2. imports
   3. attributes
   4. `struct`
   5. `enum`
   6. `type`
   7. `fn`
   8. dependent `enum` and `struct` declarations
   9. dependent variant `where` clauses
   10. `impl`
   11. blocks
   12. `let`
   13. `if`
   14. `match`
   15. `for`, `while`, `loop`
   16. calls, field access, indexing
   17. explicit, implicit, auto, and erased binders
   18. holes like `?missing`
   19. `rewrite`
   20. `impossible`
   21. `total` and `partial`
   22. `requires`, `ensures`, `where`
2. Реализовать operator precedence parser для выражений.
3. Добавить error recovery внутри item/block/expression.
4. Сохранить AST достаточно богатым для formatter.
5. Добавить parser snapshot-тесты.

Definition of done:

1. Пример из `SPEC.md` парсится.
2. Ошибки в одном item не ломают парсинг всего файла.
3. AST можно сериализовать в debug-вид для тестов.

## 6. Этап 3: Formatter

Цель: стабилизировать синтаксис до появления сложной семантики.

Задачи:

1. Реализовать pretty-printer из AST.
2. Зафиксировать правила:
   1. 4 пробела для отступов.
   2. trailing commas в многострочных структурах.
   3. одно item-разделение пустой строкой.
3. Добавить `avtan fmt --check`.
4. Добавить round-trip тесты: parse -> format -> parse.

Definition of done:

1. Formatter идемпотентен.
2. `avtan fmt --check` возвращает non-zero при отличиях.

## 7. Этап 4: Name Resolution И Пакеты

Цель: превратить AST в HIR с разрешенными именами.

Текущее состояние:

1. Сделано: добавлены `hir` и `resolve` модули со skeleton HIR, `SymbolId`,
   `BinderId`, `ScopeId`, type/value namespaces и первичной symbol table.
2. Сделано: `resolve` понижает top-level items, enum variants, generic binders,
   function params и Pi-type binders; `<T>` становится implicit type binder,
   `<const N: Nat>` становится erased value binder.
3. Сделано: `avtan resolve <file.avtn>` печатает symbol table для ручной
   проверки.
4. Осталось: реальный lookup путей, imports/aliases, package graph, multi-file
   scopes и диагностики unknown names.

Задачи:

1. Реализовать `avtan.toml`.
2. Добавить package graph.
3. Разрешать локальные имена, imports, aliases и `pub`.
4. Разрешать type namespace и value namespace отдельно.
5. Детектировать циклы импортов.
6. Подготовить symbol table для type checker.

Definition of done:

1. Несколько `.avtn` файлов в одном пакете видят друг друга.
2. Ошибка неизвестного имени указывает ближайшие похожие имена.
3. Циклический импорт диагностируется.

## 8. Этап 5: Dependent Core IR

Цель: реализовать маленькое ядро, в которое будет elaboration всего surface
языка.

Задачи:

1. Реализовать core terms:
   1. variables
   2. globals
   3. universes `Type level`
   4. Pi types
   5. Sigma types
   6. lambdas
   7. applications
   8. lets
   9. constructors
   10. case trees
   11. metavariables
   12. erased binders
2. Реализовать context и telescope.
3. Реализовать universe levels и constraints.
4. Добавить pretty-printer core terms для diagnostics.
5. Добавить builtins: `Type`, `Nat`, equality, `Refl`.

Definition of done:

1. Core может представить `id`, `Nat`, `Vect`, `head`.
2. Все binders имеют explicit/implicit/auto/erased режим.
3. Core terms печатаются с человекочитаемыми именами.

## 9. Этап 6: Elaboration

Цель: переводить Rust-like surface syntax в полный dependent core.

Задачи:

1. Реализовать bidirectional elaboration:
   1. checking mode
   2. synthesis mode
2. Вставлять implicit arguments.
3. Создавать metavariables для holes и невыведенных аргументов.
4. Поддержать typed holes `?name`.
5. Elaborate:
   1. functions
   2. lambdas
   3. applications
   4. dependent function types
   5. dependent enum/struct declarations
   6. pattern matches
   7. `rewrite`
   8. `impossible`
6. Репортить unsolved holes с context и expected type.

Definition of done:

1. `fn id<T>(x: T) -> T = x` elaborates.
2. `head(xs)` восстанавливает implicit length index.
3. Unsolved hole показывает локальный context.

## 10. Этап 7: Normalization И Definitional Equality

Цель: сделать проверку типов зависимой от вычисления программ в типах.

Задачи:

1. Реализовать normalization by evaluation или явно выбранную альтернативу.
2. Реализовать quoting normalized values back to terms.
3. Реализовать definitional equality:
   1. beta
   2. eta для функций, если включено
   3. delta unfolding для transparent definitions
   4. iota reduction для pattern matching
4. Реализовать guarded unfolding, чтобы diagnostics не разворачивали весь мир.
5. Реализовать unification для elaboration metavariables.

Definition of done:

1. `plus(Z, n)` и `n` считаются equal после normalization.
2. `Refl` принимается только когда стороны definitionally equal.
3. Ошибки equality показывают нормализованные формы.

## 11. Этап 8: Dependent Type Checker

Цель: проверять полные зависимые типы, а не отдельный набор const-параметров.

Задачи:

1. Проверять universes и не допускать `Type : Type`.
2. Проверять Pi/Sigma types.
3. Проверять dependent functions.
4. Проверять dependent enum variants with index-refining `where` clauses.
5. Проверять equality type и `Refl`.
6. Проверять `rewrite proof in expr`.
7. Проверять implicit, auto и erased arguments.
8. Проверять `requires`/`ensures` как dependent propositions.

Definition of done:

1. `Vect<A, n>` типизируется как type family.
2. `head: Vect<A, S(n)> -> A` не требует runtime bounds check.
3. Неверный индекс длины дает type error до Go lowering.

## 12. Этап 9: Coverage И Totality

Цель: гарантировать soundness вычислений, используемых в типах и proofs.

Задачи:

1. Реализовать coverage checking для dependent pattern matching.
2. Поддержать `impossible` branches.
3. Реализовать totality checker:
   1. structural recursion
   2. lexicographic recursion
   3. mutual recursion через size-change analysis
4. Разделить `total fn` и `partial fn`.
5. Запретить использование `partial fn` в types/proofs/erased computation.
6. Добавить diagnostics для non-covering и non-terminating definitions.

Definition of done:

1. Неполный `match` в total function диагностируется.
2. Очевидная structural recursion принимается.
3. General recursion разрешена только runtime-only `partial fn`.

## 13. Этап 10: Proof Syntax, Equality И Search

Цель: дать пользовательский Idris-like proof experience поверх dependent core.

Задачи:

1. Добавить `proof fn` как total erased function.
2. Добавить equality proofs через `Refl`.
3. Добавить `rewrite`.
4. Добавить `ghost let` как erased let.
5. Добавить `prove expr` как проверку proposition expression.
6. Добавить `{auto p: P}` и ограниченный proof search.
7. Поддержать `#[test] proof fn`.

Definition of done:

1. Proof-код не попадает в Go.
2. Proof-код не может вызвать IO/spawn/unsafe.
3. `plus_zero_right` доказывается pattern matching + rewrite.

## 14. Этап 11: Erasure

Цель: получить runtime-only IR перед Go backend.

Задачи:

1. Удалять:
   1. types
   2. proofs
   3. erased arguments
   4. implicit-only evidence
   5. type-level indices
2. Сохранять runtime-relevant dependent values.
3. Проверять, что erased values не влияют на runtime control flow.
4. Представить erased IR отдельно от Core.
5. Добавить тесты `core -> erased`.

Definition of done:

1. `Vect<A, n>` runtime-представление не содержит proof-only `n`, если он не
   нужен в runtime.
2. Erased proof branch не влияет на Go output.
3. Go backend получает только erased IR.

## 15. Этап 12: Go Backend v1

Цель: сгенерировать читаемый Go из erased runtime IR.

Задачи:

1. Создать Go AST:
   1. package
   2. imports
   3. structs
   4. interfaces
   5. functions
   6. statements
   7. expressions
2. Реализовать printer Go AST.
3. Прогонять результат через `gofmt`.
4. Lowering:
   1. Avtan package -> Go package
   2. `str` -> `string`
   3. numeric primitives -> Go numeric types
   4. erased structs/enums -> Go structs/interfaces
   5. fieldless enums -> Go constants
   6. payload enums -> interface + variant structs
   7. `Result<T, E>` -> `(T, error)` для Go-friendly функций
   8. `?` -> early return
5. Добавить integration-тесты: `.avtn` -> erased IR -> `.go` -> `go test`.

Definition of done:

1. Компилятор генерирует Go-пакет из erased IR.
2. Сгенерированный код проходит `gofmt`.
3. `Nat`/`Vect` proof code не попадает в Go.

## 16. Этап 13: Ownership И Borrowing v1

Цель: ввести Rust-подобную безопасность без попытки полностью скопировать Rust.

Задачи:

1. Реализовать move tracking.
2. Добавить `Copy` для primitives.
3. Проверять use-after-move.
4. Добавить `&T` и `&mut T`.
5. Проверять правило:
   1. много immutable borrows
   2. один mutable borrow
   3. mutable borrow эксклюзивен
6. Добавить простую lifetime inference внутри функции.
7. Подготовить `Send` и `Sync` marker traits для concurrency.

Definition of done:

1. Use-after-move диагностируется.
2. Одновременный `&mut` и `&` запрещен.
3. Простые borrow-программы lower-ятся в Go pointers/slices.

## 17. Этап 14: Effects

Цель: сделать side effects видимыми и пригодными для proof checker.

Задачи:

1. Представить effects в HIR.
2. Поддержать `effects(IO, Spawn, Net, Clock, Unsafe)`.
3. Инферить effects внутри пакета.
4. Требовать явные effects на public API.
5. Запретить effects в `proof fn`.
6. Пробрасывать effects через calls.

Definition of done:

1. Pure function не может вызвать IO-функцию.
2. Public function без нужного effects получает диагностику.
3. Proof checker опирается на effects.

## 18. Этап 15: Concurrency v1

Цель: реализовать встроенные примитивы многопоточности поверх Go.

Задачи:

1. Добавить типы:
   1. `Task<T, E>`
   2. `TaskGroup<E>`
   3. `CancelToken`
   4. `Chan<T>`
   5. `SendChan<T>`
   6. `RecvChan<T>`
2. Реализовать `spawn expr`.
3. Проверять `Send` для moved captures.
4. Проверять `Sync` для shared captures.
5. Lower `spawn` в goroutine + one-shot result channel.
6. Lower `chan<T>(capacity = N)` в Go channel.
7. Lower `select` в Go `select`.
8. Реализовать basic `TaskGroup` через `context.Context` + `sync.WaitGroup`.
9. Добавить cancellation propagation.

Definition of done:

1. `spawn` возвращает awaitable task.
2. Ошибка в task может быть возвращена вызывающему коду.
3. `select` компилируется в валидный Go `select`.
4. Нельзя отправить non-`Send` значение в другой task.

## 19. Этап 16: Traits И Impl

Цель: добавить пользовательские абстракции после стабилизации backend.

Задачи:

1. Реализовать trait definitions.
2. Реализовать impl blocks.
3. Проверять trait method signatures.
4. Реализовать generic trait bounds.
5. Lower object-safe traits в Go interfaces.
6. Для non-object-safe traits выбрать dictionary passing или
   monomorphization.
7. Добавить derives для базовых marker traits.

Definition of done:

1. `T: Display` проверяется.
2. Object-safe trait можно передать как interface-like value.
3. Неполный impl дает диагностику.

## 20. Этап 17: Go Interop

Цель: позволить Avtan-коду использовать существующие Go-пакеты.

Задачи:

1. Поддержать `import go "path" as alias`.
2. Поддержать `extern go { ... }`.
3. Маппить `(T, error)` в `Result<T, error>`.
4. Маппить `error` в `Result<(), error>`.
5. Описывать внешние Go-типы как opaque.
6. Требовать ручные `Send`/`Sync` declarations для extern types.
7. Генерировать Go imports без конфликтов имен.

Definition of done:

1. Можно вызвать `net/http` через explicit binding.
2. Go errors корректно становятся Avtan `Result`.
3. Неверная interop-сигнатура диагностируется.

## 21. Этап 18: Standard Library v1

Цель: дать минимальный набор типов, которые нужны языку.

Задачи:

1. Реализовать compiler-known definitions:
   1. `Type`
   2. `Nat`
   3. propositional equality and `Refl`
   4. `Dec<P>`
   5. `Fin<n>`
   6. `Vect<T, n>`
   7. `Option<T>`
   8. `Result<T, E>`
   9. `Vec<T>`
   10. `Task<T, E>`
   11. `Chan<T>`
   12. `Mutex<T>`
   13. `Atomic<T>`
2. Решить, какие типы являются source-level Avtan, а какие intrinsic.
3. Добавить Go runtime support package, если прямой Go lowering недостаточен.
4. Добавить prelude.

Definition of done:

1. Обычная программа не требует ручного импорта `Result`.
2. Runtime support versioned вместе с compiler.
3. Stdlib покрыта integration-тестами.

## 22. Этап 19: Test Runner

Цель: сделать `avtan test` полезным для компилятора и пользователей.

Задачи:

1. Собирать `#[test] fn`.
2. Собирать `#[test] proof fn`.
3. Генерировать Go `_test.go`.
4. Запускать `go test`.
5. Собирать diagnostics для compile-fail tests.
6. Добавить fixtures:
   1. lexer
   2. parser
   3. typecheck-pass
   4. typecheck-fail
   5. go-run
   6. proof-pass
   7. proof-fail

Definition of done:

1. `avtan test` запускает runtime tests.
2. Proof tests проверяются без Go runtime.
3. Compile-fail fixtures проверяют конкретные error codes.

## 23. Этап 20: Async И Cancellation

Цель: стабилизировать async-синтаксис поверх Go-модели.

Задачи:

1. Добавить `async fn`.
2. Добавить `.await`.
3. Запретить unsafe mutable borrow across await.
4. Lower async calls в blocking или task-based Go code.
5. Интегрировать `CancelToken` с `context.Context`.
6. Добавить `clock.sleep`, `clock.after`, `clock.now`.

Definition of done:

1. Async-функция может быть spawned.
2. Cancellation доходит до child tasks.
3. Borrow checker ловит mutable borrow across await.

## 24. Этап 21: Unsafe

Цель: разрешить низкоуровневые операции только в явно помеченном коде.

Задачи:

1. Добавить `unsafe fn`.
2. Добавить `unsafe { ... }`.
3. Добавить `effects(Unsafe)`.
4. Добавить raw pointer types.
5. Разрешить unchecked refinement casts только в unsafe.
6. Скрыть Go `unsafe` за backend feature flag.

Definition of done:

1. Unsafe-вызов вне unsafe block запрещен.
2. Public unsafe API требует явный `Unsafe` effect.
3. Backend не генерирует Go `unsafe` без opt-in.

## 25. Этап 22: Оптимизация И Полировка

Цель: сделать компилятор приятным и предсказуемым.

Задачи:

1. Улучшить диагностики.
2. Добавить подсказки `did you mean`.
3. Добавить incremental cache для пакетов.
4. Добавить backend flags:
   1. readable Go
   2. optimized Go
   3. debug contracts
   4. unchecked contracts
5. Добавить LSP skeleton.
6. Добавить документацию по языку.

Definition of done:

1. Большинство ошибок указывают первопричину, а не следствие.
2. Generated Go можно читать без боли.
3. Build больших пакетов не пересобирает все без причины.

## 26. Рекомендуемый Порядок Коммитов

1. `docs: add language spec and implementation plan`
2. `compiler: add source files and diagnostics`
3. `lexer: tokenize avtn source`
4. `parser: parse dependent core surface syntax`
5. `fmt: add formatter`
6. `resolve: add packages and symbols`
7. `core: add universes pi sigma and erased binders`
8. `elab: add implicit arguments and holes`
9. `nbe: add normalization and definitional equality`
10. `types: check dependent functions and enums`
11. `totality: add coverage and termination checks`
12. `proof: add equality refl rewrite and proof tests`
13. `erase: remove proof and type-only terms`
14. `go: lower erased runtime ir`
15. `types: add result and question operator`
16. `ownership: add move and borrow checks`
17. `effects: add effect checking`
18. `concurrency: lower spawn channel select`
19. `tests: add avtan test runner`
20. `interop: add explicit go bindings`

## 27. Риски

1. Полные зависимые типы могут сделать компилятор слишком сложным.
   Решение: держать core маленьким, surface syntax elaboration-driven, а Go
   backend подключать только после erasure.
2. Ownership поверх Go может стать слишком строгим или слишком слабым.
   Решение: начать с function-local move/borrow checker.
3. Totality checker может отклонять полезные программы.
   Решение: начать со strict totality для type/proof-кода и разрешать
   `partial fn` только для runtime.
4. Payload enums в Go могут генерировать много allocation-heavy кода.
   Решение: сначала readable backend, потом alternative representations.
5. Async поверх Go может конфликтовать с Go idioms.
   Решение: сперва реализовать structured concurrency без отдельного runtime.
6. Go interop может размыть soundness.
   Решение: все extern-типы требуют явных trait/effect declarations.

## 28. Ближайшие Технические Шаги

Самый полезный следующий кусок работы:

1. Расширить parser до оставшихся item/expression форм из этапа 2.
2. Добавить `avtan parse --json` или stable snapshot формат для AST.
3. Начать этап 3: formatter поверх AST.
4. После formatter перейти к этапу 4: name resolution и package graph.
5. Подготовить fixtures для parser-pass и parser-fail тестов.

После этого можно переходить к HIR/name resolution и уже готовить вход для
dependent core elaboration.
