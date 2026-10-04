import json

import pytest

from gordian_analysis.cli import main

from conftest import write_run


def run_cli(capsys, *argv):
    rc = main(list(argv))
    out = capsys.readouterr()
    return rc, out.out, out.err


def compare_args(fx, *extra, metric="success", margin="0.01", direction="--higher-is-better"):
    return (
        "compare", "--a", str(fx / "run_a"), "--b", str(fx / "run_b"),
        "--metric", metric, "--margin", margin, direction, "--seed", "1", *extra,
    )  # fmt: skip


def test_compare_success_end_to_end(capsys, fixtures_dir, tmp_path):
    out_json = tmp_path / "out.json"
    rc, out, err = run_cli(capsys, *compare_args(fixtures_dir, "--json", str(out_json)))
    assert rc == 0 and err == ""
    # Hand values from the fixtures: n = 20 pairs; mean A = 15/20 = 0.75, mean B = 16/20 = 0.80;
    # exactly one episode differs (hard, seed 7: 0 -> 1), so mean d = 1/20 = 0.05 and
    # sd d = sqrt(((0.95)^2 + 19*(0.05)^2)/19) = sqrt(0.05) = 0.2236.
    r = json.loads(out_json.read_text())
    assert r["n_pairs"] == 20
    assert r["a"]["mean"] == pytest.approx(0.75) and r["b"]["mean"] == pytest.approx(0.80)
    assert r["mean_d"] == pytest.approx(0.05)
    assert r["sd_d"] == pytest.approx(0.05**0.5)
    assert r["bootstrap"]["seed"] == 1 and r["bootstrap"]["n_resamples"] == 10000
    assert r["category"] == "unresolved"  # interval straddles the margin and includes ~0
    assert r["noninferior"] is True
    assert r["interval_confidence"] == pytest.approx(0.90)
    assert "CATEGORY: UNRESOLVED" in out
    assert "NOT equivalence" in out
    assert "Non-inferior" in out and "seed=1" in out
    assert "higher is better, so positive d means B is better" in out
    assert r["usage_present"] == {"a": False, "b": True}
    assert "risk_coverage" in r and "b" in r["risk_coverage"] and "a" not in r["risk_coverage"]
    assert {row["class"] for row in r["per_class"]} == {"easy", "hard"}


def test_compare_cost_lower_is_better(capsys, fixtures_dir):
    rc, out, _ = run_cli(
        capsys,
        *compare_args(fixtures_dir, metric="bill_total", margin="1", direction="--lower-is-better"),
    )
    # bill_total: A = 16 (easy) / 20 (hard), B = 12 / 15 -> d = A - B = 4, 5 -> mean 4.5, sd 0.513
    assert rc == 0
    # the per-class table uses the same convention, applied once: easy 16-12 = 4, hard 20-15 = 5
    assert "easy 10 16.0000 12.0000 4.0000" in " ".join(out.split())
    assert "hard 10 20.0000 15.0000 5.0000" in " ".join(out.split())
    assert "Mean of d: 4.5" in out
    assert "lower is better, so d is negated" in out
    assert "CATEGORY: BENEFICIAL" in out


def test_compare_swapped_direction_flips_sign(capsys, fixtures_dir, tmp_path):
    f1, f2 = tmp_path / "1.json", tmp_path / "2.json"
    run_cli(capsys, *compare_args(fixtures_dir, "--json", str(f1), metric="bill_total", margin="1",
                                  direction="--lower-is-better"))  # fmt: skip
    run_cli(capsys, *compare_args(fixtures_dir, "--json", str(f2), metric="bill_total", margin="1",
                                  direction="--higher-is-better"))  # fmt: skip
    assert json.loads(f1.read_text())["mean_d"] == pytest.approx(4.5)
    assert json.loads(f2.read_text())["mean_d"] == pytest.approx(-4.5)
    assert json.loads(f2.read_text())["category"] == "harmful"


def test_interval_t_option_and_reproducible_output(capsys, fixtures_dir):
    _, t_out, _ = run_cli(capsys, *compare_args(fixtures_dir, "--interval", "t"))
    assert "Interval used for the category: t" in t_out
    _, o1, _ = run_cli(capsys, *compare_args(fixtures_dir))
    _, o2, _ = run_cli(capsys, *compare_args(fixtures_dir))
    assert o1 == o2


def test_seed_and_direction_are_required(capsys, fixtures_dir):
    base = ["compare", "--a", str(fixtures_dir / "run_a"), "--b", str(fixtures_dir / "run_b"),
            "--metric", "success", "--margin", "0.01"]  # fmt: skip
    with pytest.raises(SystemExit):
        main([*base, "--higher-is-better"])  # no seed
    with pytest.raises(SystemExit):
        main([*base, "--seed", "1"])  # no direction


def test_equivalent_category_reachable_end_to_end(capsys, tmp_path):
    # 60 pairs with d = +/-0.001 alternating; the interval is tiny and inside margin 0.01.
    ra = [{"seed": s, "bill_compute": 100.0} for s in range(1, 61)]
    rb = [{"seed": s, "bill_compute": 100.0 + (0.001 if s % 2 else -0.001)} for s in range(1, 61)]
    a, b = write_run(tmp_path / "a", ra), write_run(tmp_path / "b", rb)
    rc, out, _ = run_cli(capsys, "compare", "--a", str(a), "--b", str(b), "--metric", "bill_compute",
                         "--margin", "0.01", "--higher-is-better", "--seed", "3")  # fmt: skip
    assert rc == 0 and "CATEGORY: EQUIVALENT" in out


def test_equivalent_on_tiny_sample_carries_warning(capsys, tmp_path):
    # Identical arms on 3 episodes: every d is 0, the interval collapses, and the category is
    # "equivalent" only by degenerate variance. The report must say so.
    rows_ = [{"seed": s, "bill_compute": 5.0} for s in range(1, 4)]
    a, b = write_run(tmp_path / "a", rows_), write_run(tmp_path / "b", rows_)
    _, out, _ = run_cli(capsys, "compare", "--a", str(a), "--b", str(b), "--metric", "bill_compute",
                        "--margin", "0.01", "--higher-is-better", "--seed", "1")  # fmt: skip
    assert "CATEGORY: EQUIVALENT" in out
    assert "only 3 paired episodes" in out and "identical" in out


def test_harmful_and_beneficial_end_to_end(capsys, tmp_path):
    ra = [{"seed": s, "bill_compute": 10.0 + (s % 3) * 0.1} for s in range(1, 41)]
    rb = [{"seed": s, "bill_compute": 20.0 + (s % 3) * 0.1} for s in range(1, 41)]
    a, b = write_run(tmp_path / "a", ra), write_run(tmp_path / "b", rb)
    base = ["compare", "--a", str(a), "--b", str(b), "--metric", "bill_compute", "--margin", "0.5",
            "--seed", "3"]  # fmt: skip
    _, out, _ = run_cli(capsys, *base, "--higher-is-better")
    assert "CATEGORY: BENEFICIAL" in out
    _, out, _ = run_cli(capsys, *base, "--lower-is-better")
    assert "CATEGORY: HARMFUL" in out
    assert "Non-inferior (lower limit > -margin): NO" in out


def test_load_errors_give_exit_code_2(capsys, tmp_path):
    a = write_run(tmp_path / "a", [{"seed": 1}, {"seed": 2}])
    b = write_run(tmp_path / "b", [{"seed": 1}])
    rc, out, err = run_cli(capsys, "compare", "--a", str(a), "--b", str(b), "--metric", "success",
                           "--margin", "0.01", "--higher-is-better", "--seed", "1")  # fmt: skip
    assert rc == 2 and out == "" and "unmatched" in err


def test_small_sample_warning(capsys, fixtures_dir):
    _, out, _ = run_cli(capsys, *compare_args(fixtures_dir))
    assert "WARNINGS" in out and "only 20 paired episodes" in out


def test_power_cli_matches_textbook(capsys, tmp_path):
    out_json = tmp_path / "p.json"
    rc, out, _ = run_cli(capsys, "power", "--sd", "1", "--margin", "0.5", "--alpha", "0.05",
                         "--power", "0.8", "--json", str(out_json))  # fmt: skip
    assert rc == 0 and "n (paired episodes) = 25" in out
    r = json.loads(out_json.read_text())
    assert r["n"] == 25 and r["design"] == "noninferiority" and r["sd"] == 1
    rc, out, _ = run_cli(capsys, "power", "--sd", "1", "--margin", "0.5", "--equivalence")
    assert rc == 0 and "n (paired episodes) = 35" in out and "equivalence (TOST)" in out


def test_power_cli_invalid_input(capsys):
    rc, _, err = run_cli(capsys, "power", "--sd", "-1", "--margin", "0.5")
    assert rc == 2 and "sd must be positive" in err
