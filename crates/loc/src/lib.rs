//! Player-facing strings by key. Key naming follows `docs/GLOSSARY.md`.
//!
//! A string may hold placeholders `{0}`, `{1}`, ...; [`format`] fills them.

/// Returns the string for `key`, or `None` if there is no such key.
///
/// # Examples
///
/// ```
/// assert_eq!(loc::get("window.title"), Some("Runetime"));
/// assert_eq!(loc::get("no.such.key"), None);
/// ```
pub fn get(key: &str) -> Option<&'static str> {
    Some(match key {
        "window.title" => "Runetime",

        "hint.arena" => "WASD — движение · Пробел — каст · Tab — гримуар · R — заново",
        "hint.grimoire" => "Tab — вернуться на арену",

        "grimoire.title" => "Гримуар",
        "grimoire.build" => "Собрать",
        "grimoire.not_built" => "Заклинание ещё не собрано",
        "grimoire.empty" => "Нечего кастовать: в гримуаре нет заклинаний",
        "grimoire.build.success" => "Собрано: {0}",
        "grimoire.cost.exact" => "Стоимость: {0} топлива",
        "grimoire.cost.at_most" => "Стоимость: не больше {0} топлива",
        "grimoire.cost.loop" => "Стоимость зависит от числа итераций цикла",
        "grimoire.error" => "{0} · строка {1}, столбец {2}: {3}",

        "journal.title" => "Журнал",
        "journal.empty" => "Кастов ещё не было",
        "journal.summary" => "Топливо: {0} из {1} · инструкций: {2}",
        "journal.ok" => "Каст завершён без ошибок",
        "journal.error" => "{0} на инструкции {1}: {2}",
        "journal.entry" => "{0}  {1}  · осталось {2}",
        "journal.cell" => "ячейка {0}: alloc на {1}",
        "journal.cell.freed" => "ячейка {0}: alloc на {1}, free на {2}",

        "token.literal" => "«{0}»",
        "token.ident" => "имя",
        "token.number" => "число",
        "token.eof" => "конец текста",
        "expected.expression" => "выражение",
        "expected.statement" => "инструкция или «end»",
        "expected.spell" => "«spell» или конец текста",

        "error.C001.unexpected_char" => "неожиданный символ «{0}»",
        "error.C001.number_overflow" => "число не помещается в Int",
        "error.C001.invalid_number" => "сразу после числа идёт буква или _",
        "error.C001.unexpected_token" => "ожидалось {0}, найдено {1}",
        "error.C002" => "неизвестное имя {0}",
        "error.C003.builtin" => "нет встроенной функции {0}",
        "error.C003.effect" => "нет эффекта {0}",
        "error.C004.type_mismatch" => "ожидался тип {0}, получен {1}",
        "error.C004.no_value" => "{0} не возвращает значения",
        "error.C004.unused_value" => "значение {0} не используется",
        "error.C005.arg_count" => "ожидалось аргументов: {0}, передано: {1}",
        "error.C005.unknown_arg" => "нет аргумента {0}",
        "error.C005.duplicate_arg" => "аргумент {0} передан дважды",
        "error.C005.mixed_args" => "именованные и позиционные аргументы смешаны",
        "error.C005.positional_emit_arg" => "у emit аргументы только именованные",
        "error.C006" => "имя {0} уже объявлено в этой области",
        "error.C007" => "арифметика над {0} запрещена",
        "error.C008" => "больше 64 локальных переменных",
        "error.C009" => "вложенность глубже 64 уровней",

        "error.R001" => "переполнение стека кадров",
        "error.R002" => "обращение к несуществующей ячейке {0}",
        "error.R003" => {
            "чтение или запись освобождённой ячейки {0}: выделена на инструкции {1}, освобождена на инструкции {2}. Эффекты после этого возникают в точке кастующего"
        }
        "error.R004" => {
            "повторное освобождение ячейки {0}: выделена на инструкции {1}, уже освобождена на инструкции {2}"
        }
        "error.R005" => "деление на ноль",
        "error.R006" => "переполнение целого: сила эффектов этого каста с обратным знаком",
        "error.R007" => "больше 32 живых ячеек",
        "error.R008" => "топливо исчерпано",
        "error.R009" => "переполнение стека операндов",
        "error.R010" => "испорченный байткод или вход каста",
        "error.R010.arg_count" => "у заклинания должен быть ровно один параметр — цель",
        _ => return None,
    })
}

/// Returns the string for `key` with `{0}`, `{1}`, ... replaced by `args`,
/// or `None` if there is no such key.
///
/// # Examples
///
/// ```
/// assert_eq!(loc::format("error.C002", &[&"foo"]).as_deref(), Some("неизвестное имя foo"));
/// assert_eq!(loc::format("no.such.key", &[]), None);
/// ```
pub fn format(key: &str, args: &[&dyn std::fmt::Display]) -> Option<String> {
    let mut text = get(key)?.to_owned();
    for (i, arg) in args.iter().enumerate() {
        text = text.replace(&format!("{{{i}}}"), &arg.to_string());
    }
    Some(text)
}
