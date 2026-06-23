# План реализации Avtan

Status: draft 0.1

Этот файл описывает практический порядок реализации языка Avtan из `SPEC.md`.
Главный принцип: сначала получить маленький компилятор, который стабильно
парсит, типизирует и генерирует Go для ограниченного подмножества, затем
расширять его до зависимых типов, ownership-модели и встроенной многопоточности.

## 1. Целевой MVP

Первый пригодный результат:

1. CLI-команда `avtan build`.
2. Чтение `avtan.toml`.
3. Парсинг одного пакета из `.avtn` файлов.
4. Диагностики с позициями в исходнике.
5. AST для функций, структур, enum, выражений, `match`, `Result`, массивов и
   слайсов.
6. Базовая проверка типов без полного ownership checker.
7. Refinement-типы для литералов и runtime-конструкторов.
8. Минимальные `requires`-проверки.
9. Генерация читаемого Go-кода.
10. Генерация Go-тестов для `#[test]`.
11. Простые конкурентные примитивы: `spawn`, `Task`, `Chan`, `select`.

Все остальные фичи считаются post-MVP, пока этот список не работает end-to-end.

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
  resolve/
    mod.rs
  types/
    mod.rs
    ty.rs
    infer.rs
    check.rs
  proof/
    mod.rs
    obligation.rs
    solver.rs
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
3. `Type IR`: типы, обобщения, const-параметры, refinement predicates.
4. `Obligations`: список доказательств и runtime-проверок.
5. `Go AST`: backend-представление, которое затем печатается в `.go`.

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

Цель: получить полное синтаксическое дерево для MVP-подмножества.

Задачи:

1. Реализовать parser для:
   1. package declarations
   2. imports
   3. attributes
   4. `struct`
   5. `enum`
   6. `type`
   7. `fn`
   8. `impl`
   9. blocks
   10. `let`
   11. `if`
   12. `match`
   13. `for`, `while`, `loop`
   14. calls, field access, indexing
   15. generic arguments
   16. `requires`, `ensures`, `where`
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

## 8. Этап 5: Базовая Type System

Цель: типизировать обычный код без зависимых доказательств.

Задачи:

1. Реализовать представление типов:
   1. primitives
   2. tuples
   3. arrays
   4. slices
   5. structs
   6. enums
   7. functions
   8. generics
   9. type aliases
2. Реализовать type inference для локальных `let`.
3. Реализовать проверку вызовов функций.
4. Реализовать проверку `if` и `match`.
5. Реализовать exhaustiveness для enum `match`.
6. Реализовать `Result<T, E>` и оператор `?`.
7. Добавить базовую проверку generic bounds без traits.

Definition of done:

1. Неверные типы дают понятные ошибки.
2. `match` по enum обязан быть исчерпывающим.
3. `?` работает только в функциях с совместимым `Result`.

## 9. Этап 6: Go Backend v1

Цель: сгенерировать читаемый Go для уже типизированного подмножества.

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
   4. structs -> Go structs
   5. fieldless enums -> Go constants
   6. payload enums -> interface + variant structs
   7. `Result<T, E>` -> `(T, error)` для Go-friendly функций
   8. `?` -> early return
5. Добавить integration-тесты: `.avtn` -> `.go` -> `go test`.

Definition of done:

1. Компилятор генерирует Go-пакет.
2. Сгенерированный код проходит `gofmt`.
3. Минимальный Avtan-тест запускается через Go `testing`.

## 10. Этап 7: Refinement Types

Цель: добавить первый полезный слой зависимых типов без solver-сложности.

Задачи:

1. Представить refinement type как base type + predicate.
2. Поддержать синтаксис:

   ```avtan
   type Port = u16 where self > 0 && self <= 65535
   ```

3. Доказывать literal assignments:

   ```avtan
   let port: Port = 8080
   ```

4. Генерировать constructor:

   ```avtan
   Port::new(raw) -> Result<Port, RefinementError>
   ```

5. Генерировать runtime checks для недоказанных runtime-значений.
6. Запретить неявное снятие refinement-типа, кроме безопасного upcast к base.

Definition of done:

1. Валидные литералы проходят compile-time.
2. Невалидные литералы дают compile-time error.
3. Runtime construction генерирует Go-проверку.

## 11. Этап 8: Const Parameters И Indexed Types

Цель: выразить инварианты длины и состояния на уровне типов.

Задачи:

1. Добавить `const N: Nat` в generics.
2. Поддержать простые type-level expressions:
   1. `N`
   2. integer literals
   3. `N + 1`
   4. `A + B`
   5. comparisons in `where`
3. Поддержать `[T; N]`.
4. Поддержать indexed structs:

   ```avtan
   struct Vec<T, const N: Nat> { ... }
   ```

5. Добавить normalization для простых Nat-выражений.
6. Добавить unification для const expressions.

Definition of done:

1. `fn push<T, const N: Nat>(Vec<T, N>, T) -> Vec<T, N + 1>` типизируется.
2. Несовместимые длины дают type error.
3. Статически известные массивы lower-ятся в Go `[N]T`.

## 12. Этап 9: Proof Obligations И Solver v1

Цель: отделить проверку типов от проверки логических обязательств.

Задачи:

1. Завести IR для obligations:
   1. preconditions
   2. postconditions
   3. refinement predicates
   4. const `where`
   5. array/slice bounds
2. Реализовать классификацию:
   1. `static`
   2. `dynamic`
   3. `rejected`
3. Реализовать встроенный solver для:
   1. boolean logic
   2. linear integer arithmetic
   3. Nat comparisons
   4. equality
   5. length equations
4. Добавить runtime assertion generation для `dynamic`.
5. Добавить `#[proof_only]`.

Definition of done:

1. Простые `requires` доказываются на compile-time.
2. Runtime-dependent `requires` превращаются в Go checks.
3. Неподдерживаемые доказательства дают честную ошибку, а не ICE.

## 13. Этап 10: Proof И Ghost Syntax

Цель: дать пользователю язык для подсказок компилятору.

Задачи:

1. Добавить `proof fn`.
2. Добавить `Proof<P>`.
3. Добавить `ghost let`.
4. Добавить `prove expr`.
5. Запретить effects внутри proof-кода.
6. Стирать proof/ghost при Go lowering.

Definition of done:

1. Proof-код не попадает в Go.
2. Proof-код не может вызвать IO/spawn/unsafe.
3. `#[test] proof fn` работает как compile-time test.

## 14. Этап 11: Ownership И Borrowing v1

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

## 15. Этап 12: Effects

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

## 16. Этап 13: Concurrency v1

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

## 17. Этап 14: Traits И Impl

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

## 18. Этап 15: Go Interop

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

## 19. Этап 16: Standard Library v1

Цель: дать минимальный набор типов, которые нужны языку.

Задачи:

1. Реализовать compiler-known definitions:
   1. `Option<T>`
   2. `Result<T, E>`
   3. `Vec<T>`
   4. `Task<T, E>`
   5. `Chan<T>`
   6. `Mutex<T>`
   7. `Atomic<T>`
2. Решить, какие типы являются source-level Avtan, а какие intrinsic.
3. Добавить Go runtime support package, если прямой Go lowering недостаточен.
4. Добавить prelude.

Definition of done:

1. Обычная программа не требует ручного импорта `Result`.
2. Runtime support versioned вместе с compiler.
3. Stdlib покрыта integration-тестами.

## 20. Этап 17: Test Runner

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

## 21. Этап 18: Async И Cancellation

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

## 22. Этап 19: Unsafe

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

## 23. Этап 20: Оптимизация И Полировка

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

## 24. Рекомендуемый Порядок Коммитов

1. `docs: add language spec and implementation plan`
2. `compiler: add source files and diagnostics`
3. `lexer: tokenize avtn source`
4. `parser: parse mvp items and expressions`
5. `fmt: add formatter`
6. `resolve: add packages and symbols`
7. `types: check primitive mvp`
8. `go: emit basic package`
9. `go: lower functions structs and enums`
10. `types: add result and question operator`
11. `proof: add refinement literal checks`
12. `proof: add runtime obligations`
13. `types: add const nat parameters`
14. `ownership: add move and borrow checks`
15. `effects: add effect checking`
16. `concurrency: lower spawn channel select`
17. `tests: add avtan test runner`
18. `interop: add explicit go bindings`

## 25. Риски

1. Полные зависимые типы могут сделать компилятор слишком сложным.
   Решение: держать solver-фрагмент ограниченным и разрешать runtime checks.
2. Ownership поверх Go может стать слишком строгим или слишком слабым.
   Решение: начать с function-local move/borrow checker.
3. Payload enums в Go могут генерировать много allocation-heavy кода.
   Решение: сначала readable backend, потом alternative representations.
4. Async поверх Go может конфликтовать с Go idioms.
   Решение: сперва реализовать structured concurrency без отдельного runtime.
5. Go interop может размыть soundness.
   Решение: все extern-типы требуют явных trait/effect declarations.

## 26. Ближайшие Технические Шаги

Самый полезный следующий кусок работы:

1. Создать `src/source`, `src/diagnostics`, `src/lexer`.
2. Добавить `TokenKind`, `Token`, `Span`, `SourceFile`.
3. Написать lexer для identifiers, keywords, literals и punctuation.
4. Добавить CLI-команду `avtan lex <file>`.
5. Покрыть lexer snapshot-тестами.

После этого можно переходить к parser и уже быстро получать обратную связь по
синтаксису языка.
