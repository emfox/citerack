"""The checkers are code too: a checker that quietly stops checking is worse."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_commits
import check_commit_scope
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


class CommitScopeTest(unittest.TestCase):
    """A feature crosses crates, and the checker only reports the union of them."""

    def test_a_feature_across_the_crates_is_one_change(self) -> None:
        for subject, lines in (
            ("feat(bib): write a citation key",
             {"crates/bib": 392, "crates/app": 316, "crates/store": 281}),
            ("feat(index): answer a query from posting lists",
             {"crates/index": 749, "crates/app": 20}),
            ("feat(app): import an entry from a DOI",
             {"crates/app": 522, "crates/store": 303, "crates/bib": 289}),
        ):
            self.assertEqual(check_commit_scope.review(subject, lines), [], subject)

    def test_a_subject_written_before_the_work_is_reported(self) -> None:
        # The subject is named for where a change lands, so a scope that holds
        # only the receipts is a message written before the lines arrived.
        problems = check_commit_scope.review(
            "fix(bib): measure unsaved work against the saved entry",
            {"crates/store": 72, "crates/bib": 62})
        self.assertTrue(any("bulk of the change" in p for p in problems))

    def test_the_window_and_the_workflow_in_one_commit_are_two_changes(self) -> None:
        problems = check_commit_scope.review(
            "fix(app): load the shader compiler at startup",
            {"crates/app": 92, "scripts": 64, ".github": 52})
        self.assertTrue(any("nothing in the subject accounts" in p for p in problems))

    def test_one_area_beside_the_scope_is_not_a_second_change(self) -> None:
        # A behaviour and its test, or a source and its generated lock, live in
        # different places and are one change.
        self.assertEqual(
            check_commit_scope.review(
                "chore(repo): keep sample libraries as they arrived",
                {"scripts": 150, "docs": 76}),
            [])

    def test_a_commit_that_touches_one_area_is_left_alone(self) -> None:
        self.assertEqual(
            check_commit_scope.review("feat(bib): read nested braces",
                                      {"crates/bib": 400, "docs": 12}),
            [])

    def test_a_scope_reaches_the_layers_beside_it(self) -> None:
        self.assertEqual(check_commit_scope.neighbours("store"),
                         {"crates/index", "crates/app"})
        self.assertEqual(check_commit_scope.neighbours("bib"), {"crates/index"})
        self.assertEqual(len(check_commit_scope.neighbours("repo")), 4)


class LanguageTest(unittest.TestCase):
    def test_ascii_text_has_no_cjk(self) -> None:
        self.assertIsNone(lang.first_cjk("a plain line"))

    def test_an_ideograph_is_found(self) -> None:
        self.assertIsNotNone(lang.first_cjk("mixed \u4e2d\u6587"))

    def test_a_fullwidth_colon_is_found(self) -> None:
        self.assertIsNotNone(lang.first_cjk("key: value\uff1a"))

    def test_a_translated_document_is_not_exempt(self) -> None:
        # There is one document per subject, so a translated copy is prose like any
        # other and has to be English.
        self.assertFalse(lang.is_allowlisted("docs/Guide.zh-CN.md"))
        self.assertFalse(lang.is_allowlisted("README.zh-CN.md"))

    def test_an_english_document_is_not_exempt(self) -> None:
        self.assertFalse(lang.is_allowlisted("docs/Architecture.md"))

    def test_a_locale_catalogue_is_prose_like_any_other(self) -> None:
        # The words live in a Rust type, so a folder of translated strings would be
        # a second copy of them rather than the data they are translated into.
        self.assertFalse(lang.is_allowlisted("locales/zh-CN/main.ftl"))

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
