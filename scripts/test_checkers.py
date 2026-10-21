"""The checkers are code too: a checker that quietly stops checking is worse."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_commits
import check_language
import check_language as lang


class CommitSubjectTest(unittest.TestCase):
    def test_a_well_formed_subject_passes(self) -> None:
        self.assertEqual(check_commits.check("feat(bib): read nested braces", ""), [])

    def test_a_subject_without_a_scope_passes(self) -> None:
        self.assertEqual(check_commits.check("docs: describe the layout", ""), [])

    def test_an_unknown_type_fails(self) -> None:
        problems = check_commits.check("improve(bib): read nested braces", "")
        self.assertTrue(any("not one of" in p for p in problems))

    def test_a_trailing_period_fails(self) -> None:
        problems = check_commits.check("fix(bib): read nested braces.", "")
        self.assertIn("subject has a trailing period", problems)

    def test_a_long_subject_fails(self) -> None:
        problems = check_commits.check("fix(bib): " + "x" * 80, "")
        self.assertTrue(any("bytes" in p for p in problems))

    def test_a_body_of_three_lines_fails(self) -> None:
        problems = check_commits.check("fix(bib): tighten the loop", "one\ntwo\nthree")
        self.assertTrue(any("body has 3 lines" in p for p in problems))

    def test_a_body_of_two_lines_passes(self) -> None:
        self.assertEqual(check_commits.check("fix(bib): tighten the loop", "one\ntwo"), [])

    def test_cjk_in_a_message_fails(self) -> None:
        problems = check_commits.check("fix(bib): \u4fee\u6b63", "")
        self.assertIn("message contains CJK", problems)


class LanguageTest(unittest.TestCase):
    def test_ascii_text_has_no_cjk(self) -> None:
        self.assertIsNone(lang.first_cjk("a plain line"))

    def test_an_ideograph_is_found(self) -> None:
        self.assertIsNotNone(lang.first_cjk("mixed \u4e2d\u6587"))

    def test_a_fullwidth_colon_is_found(self) -> None:
        self.assertIsNotNone(lang.first_cjk("key: value\uff1a"))

    def test_a_chinese_document_name_is_exempt(self) -> None:
        self.assertTrue(lang.is_allowlisted("docs/Guide.zh-CN.md"))

    def test_an_english_document_is_not_exempt(self) -> None:
        self.assertFalse(lang.is_allowlisted("docs/Architecture.md"))

    def test_locale_catalogues_are_exempt(self) -> None:
        self.assertTrue(lang.is_allowlisted("locales/zh-CN/main.ftl"))

    def test_build_output_is_skipped(self) -> None:
        self.assertTrue(lang.is_allowlisted("target/debug/build/x.rs"))

    def test_the_policy_document_is_exempt_for_quoting_the_rule(self) -> None:
        self.assertTrue(check_language.is_allowlisted("docs/Conventions.md"))


class TestModuleLiteralTest(unittest.TestCase):
    """A test may hold CJK in a literal, and only there."""

    SOURCE = (
        "fn split() {}\n"
        "#[cfg(test)]\n"
        "mod tests {\n"
        "    #[test]\n"
        "    fn one() {\n"
        '        assert_eq!(w("\u4e2d\u6587"), 1);\n'
        "    }\n"
        "}\n"
    )

    def lines(self, source: str) -> list[str]:
        return source.splitlines()

    def test_the_literal_of_a_test_module_is_exempt(self) -> None:
        kept = lang.without_literals(self.lines(self.SOURCE)[5])
        self.assertIsNone(lang.first_cjk(kept))

    def test_a_comment_in_a_test_module_still_fails(self) -> None:
        line = '        assert_eq!(w("\u4e2d\u6587"), 1); // \u4e2d\u6587'
        self.assertIsNotNone(lang.first_cjk(lang.without_literals(line)))

    def test_the_module_body_is_marked_up_to_its_closing_brace(self) -> None:
        inside = lang.test_module_lines(self.lines(self.SOURCE))
        self.assertIn(5, inside, "the assertion line")
        self.assertNotIn(1, inside, "the function before the module")
        self.assertNotIn(8, inside, "the line after the module")

    def test_a_literal_outside_a_test_module_is_not_exempt(self) -> None:
        inside = lang.test_module_lines(self.lines(self.SOURCE))
        self.assertNotIn(1, inside)


if __name__ == "__main__":
    unittest.main()
