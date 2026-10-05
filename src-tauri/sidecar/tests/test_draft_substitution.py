# ABOUTME: Tests for services/draft_substitution.py's placeholder rules and presence check.
# ABOUTME: One test per rule and exclusion; see agent_docs for each rule's mutation verification.

from models.generated import DraftField
from services import draft_substitution as sub


def field(key, label, value, type=None, aliases=None):
    return DraftField(key=key, label=label, value=value, type=type, aliases=aliases)


# --- Rule 1: bracket matches a field's label or alias, exactly ---

def test_rule1_matches_field_label():
    f = field("employeeName", "Employee Name", "Tester")
    assert sub.classify_bracket("Employee Name", [f]) is f


def test_rule1_matches_an_alias_identically_to_the_label():
    f = field("position", "Position", "Role", aliases=["Role", "Job Title", "Employee Title"])
    assert sub.classify_bracket("Employee Title", [f]) is f
    assert sub.classify_bracket("JOB TITLE", [f]) is f


def test_rule1_does_not_match_a_longer_descriptive_phrase():
    f = field("position", "Position", "Role", aliases=["Role"])
    assert sub.classify_bracket("Detailed description of primary role", [f]) is None


# --- Rule 3: "[Label: anything]" -- the label alone decides ---

def test_rule3_label_colon_value_matches_on_the_label_alone():
    f = field("startDate", "Start Date", "2026-09-19", type="date")
    assert sub.classify_bracket("Start Date: 2026-09-19", [f]) is f


def test_rule3_ignores_an_unrelated_label_before_the_colon():
    f = field("startDate", "Start Date", "2026-09-19", type="date")
    assert sub.classify_bracket("SUBJECT: EMPLOYMENT AGREEMENT", [f]) is None


# --- Rule 0: bracket is just the field's own value ---

def test_rule0_matches_the_bare_value():
    f = field("employeeName", "Employee Name", "Tester")
    assert sub.classify_bracket("Tester", [f]) is f


def test_rule0_name_suffix_only_applies_to_name_like_labels():
    name_field = field("employeeName", "Employee Name", "Tester")
    assert sub.classify_bracket("Tester's Name", [name_field]) is name_field

    duration_field = field("duration", "Duration", "2 years", type="freetext")
    # "2 years Name" is nonsensical and must not match -- the "Name" suffix
    # is only generated for fields whose own label is name-like.
    assert sub.classify_bracket("2 years Name", [duration_field]) is None


# --- Rule 2: "<another field's value> <this field's label/alias>" ---

def test_rule2_value_plus_another_fields_label():
    name = field("employeeName", "Employee Name", "Tester")
    position = field("position", "Position", "Role")
    assert sub.classify_bracket("Tester Position", [name, position]) is position


def test_rule2_value_plus_another_fields_alias():
    provider = field("serviceProvider", "Service Provider", "Acme Consulting")
    scope = field("scopeOfWork", "Scope of Work", "Website redesign", type="freetext", aliases=["Scope", "Services"])
    assert sub.classify_bracket("Acme Consulting Scope", [provider, scope]) is scope


def test_rule2_does_not_pair_a_field_with_its_own_label():
    """"[Tester Employee Name]" isn't employeeName qualifying itself -- Rule 2
    only pairs a value with a DIFFERENT field's label."""
    name = field("employeeName", "Employee Name", "Tester")
    assert sub.classify_bracket("Tester Employee Name", [name]) is None


# --- No false positives on an unrelated attribute (exact match, not substring) ---

def test_value_literal_is_exact_not_substring():
    """"[Tester's Address]" must not match employeeName just because "tester"
    is a substring of the normalised bracket -- classify_bracket checks SET
    MEMBERSHIP against precomputed candidates, not containment."""
    f = field("employeeName", "Employee Name", "Tester")
    assert sub.classify_bracket("Tester's Address", [f]) is None
    assert sub.classify_bracket("Tester's Signature", [f]) is None


def test_qualified_label_is_exact_not_substring():
    name = field("employeeName", "Employee Name", "Tester")
    position = field("position", "Position", "Role")
    # An extra trailing word breaks the exact "<value> <label>" match.
    assert sub.classify_bracket("Tester Position Signature", [name, position]) is None


def test_bare_title_is_never_matched_even_though_position_has_aliases():
    """Bare "[Title]" is at least as often the EMPLOYER's signatory title as
    the employee's -- it must stay an honest blank regardless of position's
    aliases, which deliberately exclude bare "Title"."""
    position = field("position", "Position", "Role", aliases=["Role", "Job Title", "Employee Title"])
    assert sub.classify_bracket("Title", [position]) is None
    assert sub.classify_bracket("Employer Title", [position]) is None


# --- apply(): end-to-end substitution, including date formatting ---

def test_apply_inserts_prose_date_for_date_typed_fields():
    f = field("startDate", "Start Date", "2026-09-19", type="date")
    new_text, subs = sub.apply("Commencing on [Start Date].", [f])
    assert new_text == "Commencing on 19 September 2026."
    assert subs == [{"placeholder": "[Start Date]", "key": "startDate", "inserted": "19 September 2026"}]


def test_apply_inserts_non_date_values_verbatim():
    f = field("salary", "Salary", "100000")
    new_text, _ = sub.apply("A salary of $[Salary].", [f])
    assert new_text == "A salary of $100000."


def test_apply_leaves_unmatched_placeholders_untouched():
    f = field("employeeName", "Employee Name", "Tester")
    new_text, subs = sub.apply("See [Employer Name] and [Tester].", [f])
    assert new_text == "See [Employer Name] and Tester."
    assert subs == [{"placeholder": "[Tester]", "key": "employeeName", "inserted": "Tester"}]


# --- all_present(): the retry-once decision ---

def test_all_present_normalises_dates():
    f = field("startDate", "Start Date", "2026-09-19", type="date")
    assert sub.all_present("Effective 19 September 2026.", [f]) == {"startDate": True}
    assert sub.all_present("Effective some other day.", [f]) == {"startDate": False}


def test_all_present_normalises_amounts():
    f = field("salary", "Salary", "100000")
    assert sub.all_present("A salary of $100,000 per year.", [f]) == {"salary": True}
    assert sub.all_present("A salary to be agreed.", [f]) == {"salary": False}


def test_all_present_freetext_accepts_a_paraphrase():
    f = field("scopeOfWork", "Scope of Work", "Website redesign and maintenance", type="freetext")
    paraphrased = "The Contractor shall redesign and maintain the Client's website."
    assert sub.all_present(paraphrased, [f]) == {"scopeOfWork": True}


def test_all_present_freetext_still_flags_a_genuine_omission():
    f = field("scopeOfWork", "Scope of Work", "Website redesign and maintenance", type="freetext")
    assert sub.all_present("This Agreement is for general consulting services.", [f]) == {"scopeOfWork": False}


def test_all_present_exact_field_requires_the_literal_value():
    f = field("employeeName", "Employee Name", "Tester")
    assert sub.all_present("Employed: Tester.", [f]) == {"employeeName": True}
    assert sub.all_present("Employed: the Employee.", [f]) == {"employeeName": False}
