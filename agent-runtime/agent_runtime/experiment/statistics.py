"""Statistical significance tests for experiment analysis.

Provides pure-numpy implementations of:
- Welch's t-test (two-sample, unequal variance)
- Chi-square test of independence
- Mann-Whitney U test (non-parametric alternative)
- Effect size (Cohen's d)

No scipy dependency required — all calculations use numpy only.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Any

import numpy as np


@dataclass
class TestResult:
    """Result of a statistical test.

    Attributes:
        statistic: The test statistic value (t-value, chi-square value, or U-value).
        p_value: Two-tailed p-value.
        significant: True if p_value < alpha.
        alpha: Significance level used.
        test_name: Human-readable name of the test performed.
        details: Additional details (e.g., degrees of freedom).
    """

    statistic: float
    p_value: float
    significant: bool
    alpha: float
    test_name: str
    details: dict[str, Any] | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "test_name": self.test_name,
            "statistic": round(self.statistic, 6),
            "p_value": round(self.p_value, 6),
            "significant": self.significant,
            "alpha": self.alpha,
            "details": self.details or {},
        }


# ---------------------------------------------------------------------------
# Welch's t-test
# ---------------------------------------------------------------------------


def welch_t_test(
    group_a: list[float] | np.ndarray,
    group_b: list[float] | np.ndarray,
    alpha: float = 0.05,
) -> TestResult:
    """Perform Welch's t-test (two-sample, unequal variance).

    Args:
        group_a: Observations from group A (control).
        group_b: Observations from group B (treatment).
        alpha: Significance level (default 0.05).

    Returns:
        TestResult with t-statistic, p-value, and significance.
    """
    a = np.asarray(group_a, dtype=float)
    b = np.asarray(group_b, dtype=float)

    n_a, n_b = len(a), len(b)
    if n_a < 2 or n_b < 2:
        return TestResult(
            statistic=0.0,
            p_value=1.0,
            significant=False,
            alpha=alpha,
            test_name="Welch's t-test",
            details={"error": "Each group needs at least 2 observations"},
        )

    mean_a, mean_b = float(np.mean(a)), float(np.mean(b))
    var_a = float(np.var(a, ddof=1))
    var_b = float(np.var(b, ddof=1))

    se_a = var_a / n_a
    se_b = var_b / n_b
    se_sum = se_a + se_b

    if se_sum == 0.0:
        return TestResult(
            statistic=0.0,
            p_value=1.0,
            significant=False,
            alpha=alpha,
            test_name="Welch's t-test",
            details={"error": "Zero variance in both groups"},
        )

    t_stat = (mean_b - mean_a) / math.sqrt(se_sum)

    # Welch-Satterthwaite degrees of freedom
    if se_a > 0 and se_b > 0:
        df_num = se_sum ** 2
        df_denom = (se_a ** 2 / (n_a - 1)) + (se_b ** 2 / (n_b - 1))
        df = df_num / df_denom if df_denom > 0 else float(n_a + n_b - 2)
    else:
        df = float(n_a + n_b - 2)

    p_value = _two_tailed_t_p_value(abs(t_stat), df)

    return TestResult(
        statistic=t_stat,
        p_value=p_value,
        significant=p_value < alpha,
        alpha=alpha,
        test_name="Welch's t-test",
        details={
            "mean_a": round(mean_a, 4),
            "mean_b": round(mean_b, 4),
            "var_a": round(var_a, 4),
            "var_b": round(var_b, 4),
            "df": round(df, 4),
            "n_a": n_a,
            "n_b": n_b,
        },
    )


# ---------------------------------------------------------------------------
# Chi-square test
# ---------------------------------------------------------------------------


def chi_square_test(
    observed: list[list[int]] | np.ndarray,
    alpha: float = 0.05,
) -> TestResult:
    """Perform chi-square test of independence.

    Args:
        observed: Contingency table (2D array of observed frequencies).
        alpha: Significance level (default 0.05).

    Returns:
        TestResult with chi-square statistic, p-value, and significance.
    """
    obs = np.asarray(observed, dtype=float)

    if obs.ndim != 2 or obs.shape[0] < 2 or obs.shape[1] < 2:
        return TestResult(
            statistic=0.0,
            p_value=1.0,
            significant=False,
            alpha=alpha,
            test_name="Chi-square test",
            details={"error": "Need at least a 2x2 contingency table"},
        )

    row_totals = obs.sum(axis=1, keepdims=True)
    col_totals = obs.sum(axis=0, keepdims=True)
    grand_total = obs.sum()

    if grand_total == 0:
        return TestResult(
            statistic=0.0,
            p_value=1.0,
            significant=False,
            alpha=alpha,
            test_name="Chi-square test",
            details={"error": "Empty contingency table"},
        )

    expected = row_totals * col_totals / grand_total

    # Chi-square statistic
    chi2 = float(np.sum((obs - expected) ** 2 / expected))

    # Degrees of freedom
    df = (obs.shape[0] - 1) * (obs.shape[1] - 1)

    p_value = _chi2_survival(chi2, df)

    return TestResult(
        statistic=chi2,
        p_value=p_value,
        significant=p_value < alpha,
        alpha=alpha,
        test_name="Chi-square test of independence",
        details={
            "df": df,
            "rows": obs.shape[0],
            "cols": obs.shape[1],
            "grand_total": int(grand_total),
        },
    )


def chi_square_goodness_of_fit(
    observed: list[int] | np.ndarray,
    expected: list[float] | np.ndarray | None = None,
    alpha: float = 0.05,
) -> TestResult:
    """Perform chi-square goodness-of-fit test.

    Args:
        observed: Observed frequencies.
        expected: Expected frequencies (uniform if None).
        alpha: Significance level.

    Returns:
        TestResult with chi-square statistic and p-value.
    """
    obs = np.asarray(observed, dtype=float)
    if expected is None:
        expected = np.full_like(obs, obs.sum() / len(obs))
    exp = np.asarray(expected, dtype=float)

    if len(obs) < 2:
        return TestResult(
            statistic=0.0,
            p_value=1.0,
            significant=False,
            alpha=alpha,
            test_name="Chi-square goodness-of-fit",
            details={"error": "Need at least 2 categories"},
        )

    chi2 = float(np.sum((obs - exp) ** 2 / exp))
    df = len(obs) - 1
    p_value = _chi2_survival(chi2, df)

    return TestResult(
        statistic=chi2,
        p_value=p_value,
        significant=p_value < alpha,
        alpha=alpha,
        test_name="Chi-square goodness-of-fit",
        details={"df": df, "categories": len(obs)},
    )


# ---------------------------------------------------------------------------
# Mann-Whitney U test (non-parametric)
# ---------------------------------------------------------------------------


def mann_whitney_u_test(
    group_a: list[float] | np.ndarray,
    group_b: list[float] | np.ndarray,
    alpha: float = 0.05,
) -> TestResult:
    """Perform Mann-Whitney U test (Wilcoxon rank-sum test).

    Non-parametric alternative to t-test when normality is not assumed.

    Args:
        group_a: Observations from group A.
        group_b: Observations from group B.
        alpha: Significance level.

    Returns:
        TestResult with U-statistic and approximate p-value.
    """
    a = np.asarray(group_a, dtype=float)
    b = np.asarray(group_b, dtype=float)
    n_a, n_b = len(a), len(b)

    if n_a < 1 or n_b < 1:
        return TestResult(
            statistic=0.0,
            p_value=1.0,
            significant=False,
            alpha=alpha,
            test_name="Mann-Whitney U test",
            details={"error": "Each group needs at least 1 observation"},
        )

    # Combine and rank
    combined = np.concatenate([a, b])
    n = n_a + n_b

    # Compute ranks (average ties)
    order = np.argsort(combined)
    ranks = np.empty_like(order, dtype=float)
    ranks[order] = np.arange(1, n + 1, dtype=float)

    # Handle ties: assign average rank
    sorted_vals = combined[order]
    i = 0
    while i < n:
        j = i + 1
        while j < n and sorted_vals[j] == sorted_vals[i]:
            j += 1
        if j > i + 1:
            avg_rank = (i + 1 + j) / 2.0
            for k in range(i, j):
                ranks[order[k]] = avg_rank
        i = j

    rank_sum_a = float(ranks[:n_a].sum())
    u_a = rank_sum_a - n_a * (n_a + 1) / 2.0
    u_b = n_a * n_b - u_a
    u_stat = min(u_a, u_b)

    # Normal approximation for p-value
    mean_u = n_a * n_b / 2.0
    # No tie correction for simplicity
    std_u = math.sqrt(n_a * n_b * (n_a + n_b + 1) / 12.0)

    if std_u == 0:
        p_value = 1.0
    else:
        z = (u_stat - mean_u) / std_u
        p_value = 2.0 * (1.0 - _normal_cdf(abs(z)))

    return TestResult(
        statistic=u_stat,
        p_value=p_value,
        significant=p_value < alpha,
        alpha=alpha,
        test_name="Mann-Whitney U test",
        details={
            "u_a": round(u_a, 4),
            "u_b": round(u_b, 4),
            "rank_sum_a": round(rank_sum_a, 4),
            "n_a": n_a,
            "n_b": n_b,
        },
    )


# ---------------------------------------------------------------------------
# Effect size
# ---------------------------------------------------------------------------


def cohens_d(
    group_a: list[float] | np.ndarray,
    group_b: list[float] | np.ndarray,
) -> float:
    """Compute Cohen's d effect size.

    Interpretation:
        |d| < 0.2: negligible
        0.2 <= |d| < 0.5: small
        0.5 <= |d| < 0.8: medium
        |d| >= 0.8: large
    """
    a = np.asarray(group_a, dtype=float)
    b = np.asarray(group_b, dtype=float)

    mean_diff = float(np.mean(b)) - float(np.mean(a))
    n_a, n_b = len(a), len(b)

    if n_a < 2 or n_b < 2:
        return 0.0

    var_a = float(np.var(a, ddof=1))
    var_b = float(np.var(b, ddof=1))

    # Pooled standard deviation
    pooled_std = math.sqrt(((n_a - 1) * var_a + (n_b - 1) * var_b) / (n_a + n_b - 2))

    if pooled_std == 0:
        return 0.0

    return mean_diff / pooled_std


# ---------------------------------------------------------------------------
# Convenience: compare all metrics between two experiment groups
# ---------------------------------------------------------------------------


def compare_metrics(
    metrics_a: dict[str, list[float]],
    metrics_b: dict[str, list[float]],
    alpha: float = 0.05,
) -> dict[str, dict[str, Any]]:
    """Compare multiple metrics between two groups using Welch's t-test.

    Args:
        metrics_a: Dict of metric_name -> list of values for group A.
        metrics_b: Dict of metric_name -> list of values for group B.
        alpha: Significance level.

    Returns:
        Dict of metric_name -> {test_result, effect_size, recommendation}.
    """
    results: dict[str, dict[str, Any]] = {}

    for metric_name in metrics_a:
        if metric_name not in metrics_b:
            continue

        values_a = metrics_a[metric_name]
        values_b = metrics_b[metric_name]

        test = welch_t_test(values_a, values_b, alpha=alpha)
        effect = cohens_d(values_a, values_b)

        if test.significant:
            if effect > 0:
                recommendation = f"B is significantly higher (d={effect:.2f})"
            else:
                recommendation = f"A is significantly higher (d={effect:.2f})"
        else:
            recommendation = "No significant difference"

        results[metric_name] = {
            "test": test.to_dict(),
            "effect_size": round(effect, 4),
            "recommendation": recommendation,
        }

    return results


# ---------------------------------------------------------------------------
# Internal: distribution functions (pure numpy, no scipy)
# ---------------------------------------------------------------------------


def _two_tailed_t_p_value(t_abs: float, df: float) -> float:
    """Approximate two-tailed p-value from t-distribution.

    Uses the regularized incomplete beta function I_x(a, b) where
    x = df / (df + t^2), a = df/2, b = 1/2.
    """
    if t_abs > 50.0:
        return 0.0
    if df <= 0:
        return 1.0

    x = df / (df + t_abs * t_abs)
    a = df / 2.0
    b = 0.5

    return _regularized_incomplete_beta(x, a, b)


def _regularized_incomplete_beta(x: float, a: float, b: float) -> float:
    """Compute I_x(a, b) using continued fraction (Lentz's method)."""
    if x <= 0.0:
        return 0.0
    if x >= 1.0:
        return 1.0

    # Use log-beta function for numerical stability
    ln_beta = _lgamma(a) + _lgamma(b) - _lgamma(a + b)
    front = math.exp(a * math.log(x) + b * math.log(1.0 - x) - ln_beta) / a

    cf = _beta_continued_fraction(x, a, b)
    return front * cf


def _beta_continued_fraction(x: float, a: float, b: float) -> float:
    """Continued fraction for incomplete beta using Lentz's method."""
    max_iter = 200
    eps = 1e-10
    tiny = 1e-30

    qab = a + b
    qap = a + 1.0
    qam = a - 1.0

    c = 1.0
    d = 1.0 - qab * x / qap
    if abs(d) < tiny:
        d = tiny
    d = 1.0 / d
    h = d

    for m in range(1, max_iter + 1):
        m2 = 2 * m

        # Even step
        aa = m * (b - m) * x / ((qam + m2) * (a + m2))
        d = 1.0 + aa * d
        if abs(d) < tiny:
            d = tiny
        c = 1.0 + aa / c
        if abs(c) < tiny:
            c = tiny
        d = 1.0 / d
        h *= d * c

        # Odd step
        aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2))
        d = 1.0 + aa * d
        if abs(d) < tiny:
            d = tiny
        c = 1.0 + aa / c
        if abs(c) < tiny:
            c = tiny
        d = 1.0 / d
        del_val = d * c
        h *= del_val

        if abs(del_val - 1.0) <= eps:
            break

    return h


def _lgamma(x: float) -> float:
    """Log-gamma function using Lanczos approximation."""
    if x <= 0.0:
        return float("nan")

    cof = [
        76.18009172947146,
        -86.50532032941677,
        24.01409824083091,
        -1.231739572450155,
        0.1208650973866179e-2,
        -0.5395239384953e-5,
    ]
    y = x
    tmp = x + 5.5
    tmp -= (x + 0.5) * math.log(tmp)
    ser = 1.000000000190015
    for c in cof:
        y += 1.0
        ser += c / y
    return -tmp + math.log(2.5066282746310005 * ser / x)


def _chi2_survival(x: float, df: int) -> float:
    """Compute survival function (1 - CDF) for chi-square distribution.

    Uses the regularized incomplete gamma function: P(x/2, df/2).
    """
    if df <= 0:
        return 1.0
    if x <= 0:
        return 1.0

    # 1 - P(x/2, df/2) = Q(x/2, df/2)
    return 1.0 - _regularized_gamma_p(x / 2.0, df / 2.0)


def _regularized_gamma_p(x: float, a: float) -> float:
    """Regularized lower incomplete gamma function P(a, x)."""
    if x <= 0.0:
        return 0.0

    if x < a + 1.0:
        # Series expansion
        return _gamma_series(x, a)
    else:
        # Continued fraction
        return 1.0 - _gamma_cf(x, a)


def _gamma_series(x: float, a: float) -> float:
    """Series expansion for P(a, x)."""
    max_iter = 200
    eps = 1e-12

    ln_prefix = a * math.log(x) - x - _lgamma(a)
    prefix = math.exp(ln_prefix)

    term = 1.0 / a
    total = term
    for _ in range(max_iter):
        a_new = a + 1.0
        term *= x / a_new
        total += term
        if abs(term) < abs(total) * eps:
            break
        a = a_new

    return prefix * total


def _gamma_cf(x: float, a: float) -> float:
    """Continued fraction for Q(a, x) = 1 - P(a, x)."""
    max_iter = 200
    eps = 1e-12
    tiny = 1e-30

    ln_prefix = a * math.log(x) - x - _lgamma(a)
    prefix = math.exp(ln_prefix)

    b = x + 1.0 - a
    c = 1.0 / tiny
    d = 1.0 / b
    h = d

    for i in range(1, max_iter + 1):
        an = -i * (i - a)
        b += 2.0
        d = an * d + b
        if abs(d) < tiny:
            d = tiny
        c = b + an / c
        if abs(c) < tiny:
            c = tiny
        d = 1.0 / d
        del_val = d * c
        h *= del_val
        if abs(del_val - 1.0) < eps:
            break

    return prefix * h


def _normal_cdf(x: float) -> float:
    """Standard normal CDF approximation."""
    # Abramowitz and Stegun approximation
    a1 = 0.254829592
    a2 = -0.284496736
    a3 = 1.421413741
    a4 = -1.453152027
    a5 = 1.061405429
    p = 0.3275911

    sign = 1 if x >= 0 else -1
    x = abs(x) / math.sqrt(2.0)
    t = 1.0 / (1.0 + p * x)
    y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * math.exp(-x * x)

    return 0.5 * (1.0 + sign * y)
