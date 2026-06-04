"""Analysis utilities — pure computation, no API calls.

These helpers operate on data dicts returned by the SDK or loaded from
exported files. They do not make HTTP requests.

The module provides:

- Descriptive statistics (mean, variance, distribution)
- Correlation analysis (behavior vs outcomes)
- Emergence pattern detection (3+ algorithms)
- Statistical significance testing
- Cultural diversity metrics
- Network analysis helpers
"""

from __future__ import annotations

import math
from collections import Counter, defaultdict
from typing import Any


class AnalyzeModule:
    """Lightweight analysis functions for Agent World data."""

    # =====================================================================
    # Descriptive Statistics
    # =====================================================================

    @staticmethod
    def descriptive_stats(values: list[float]) -> dict[str, Any]:
        """Compute descriptive statistics for a numeric list.

        Returns count, mean, median, mode, variance, std_dev, min, max,
        range, skewness, kurtosis, quartiles (q1, q3, iqr).
        """
        n = len(values)
        if n == 0:
            return _empty_descriptive()

        sorted_vals = sorted(values)
        total = sum(values)
        mean = total / n

        # Median
        mid = n // 2
        median = (sorted_vals[mid - 1] + sorted_vals[mid]) / 2 if n % 2 == 0 else sorted_vals[mid]

        # Mode
        freq: Counter[float] = Counter(values)
        mode_val = freq.most_common(1)[0][0]

        # Variance and std
        if n > 1:
            variance = sum((v - mean) ** 2 for v in values) / (n - 1)
        else:
            variance = 0.0
        std_dev = math.sqrt(variance)

        # Quartiles
        q1 = _percentile(sorted_vals, 25)
        q3 = _percentile(sorted_vals, 75)
        iqr = q3 - q1

        # Skewness (Pearson's second coefficient)
        if std_dev > 0:
            skewness = (3 * (mean - median)) / std_dev
        else:
            skewness = 0.0

        # Kurtosis (excess kurtosis)
        if std_dev > 0 and n > 3:
            kurtosis = (
                sum((v - mean) ** 4 for v in values) / (n * std_dev ** 4) - 3.0
            )
        else:
            kurtosis = 0.0

        return {
            "count": n,
            "mean": round(mean, 6),
            "median": round(median, 6),
            "mode": mode_val,
            "variance": round(variance, 6),
            "std_dev": round(std_dev, 6),
            "min": sorted_vals[0],
            "max": sorted_vals[-1],
            "range": sorted_vals[-1] - sorted_vals[0],
            "skewness": round(skewness, 6),
            "kurtosis": round(kurtosis, 6),
            "q1": round(q1, 6),
            "q3": round(q3, 6),
            "iqr": round(iqr, 6),
        }

    @staticmethod
    def frequency_distribution(
        values: list[Any],
        *,
        bins: int | None = None,
    ) -> dict[str, Any]:
        """Compute frequency distribution for a list of values.

        For numeric values with *bins* specified, creates histogram bins.
        Otherwise counts distinct values.

        Returns counts dict, total, unique_count, and (for binned) bin_edges.
        """
        if not values:
            return {"counts": {}, "total": 0, "unique_count": 0}

        if bins is not None:
            numeric = [float(v) for v in values]
            min_v = min(numeric)
            max_v = max(numeric)
            bin_width = (max_v - min_v) / bins if bins > 0 and max_v > min_v else 1.0
            counts: dict[str, int] = defaultdict(int)
            for v in numeric:
                if max_v == min_v:
                    idx = 0
                else:
                    idx = min(int((v - min_v) / bin_width), bins - 1)
                low = round(min_v + idx * bin_width, 4)
                high = round(min_v + (idx + 1) * bin_width, 4)
                counts[f"[{low}, {high})"] += 1
            return {
                "counts": dict(counts),
                "total": len(values),
                "unique_count": len(set(values)),
                "bin_edges": [
                    round(min_v + i * bin_width, 4) for i in range(bins + 1)
                ],
            }

        counts = dict(Counter(str(v) for v in values))
        return {
            "counts": counts,
            "total": len(values),
            "unique_count": len(counts),
        }

    @staticmethod
    def group_statistics(
        data: list[dict],
        group_key: str,
        value_key: str,
    ) -> dict[str, dict[str, Any]]:
        """Compute descriptive stats per group.

        Each dict should have *group_key* and *value_key*.

        Returns ``{group: {mean, median, std_dev, count, min, max, sum}}``.
        """
        groups: dict[str, list[float]] = defaultdict(list)
        for row in data:
            gk = str(row.get(group_key, ""))
            val = row.get(value_key)
            if gk and val is not None:
                try:
                    groups[gk].append(float(val))
                except (ValueError, TypeError):
                    continue

        result: dict[str, dict[str, Any]] = {}
        for gk, vals in groups.items():
            n = len(vals)
            if n == 0:
                continue
            sorted_v = sorted(vals)
            mean = sum(vals) / n
            median = (sorted_v[n // 2 - 1] + sorted_v[n // 2]) / 2 if n % 2 == 0 else sorted_v[n // 2]
            std_dev = math.sqrt(sum((v - mean) ** 2 for v in vals) / (n - 1)) if n > 1 else 0.0
            result[gk] = {
                "count": n,
                "mean": round(mean, 6),
                "median": round(median, 6),
                "std_dev": round(std_dev, 6),
                "min": sorted_v[0],
                "max": sorted_v[-1],
                "sum": sum(vals),
            }
        return result

    # =====================================================================
    # Correlation Analysis
    # =====================================================================

    @staticmethod
    def pearson_correlation(x: list[float], y: list[float]) -> float:
        """Compute Pearson correlation coefficient between two numeric lists.

        Returns a value in [-1, 1].  Returns 0.0 if lists have fewer than
        2 elements or if either has zero variance.
        """
        n = min(len(x), len(y))
        if n < 2:
            return 0.0

        x = x[:n]
        y = y[:n]

        mean_x = sum(x) / n
        mean_y = sum(y) / n

        cov = sum((xi - mean_x) * (yi - mean_y) for xi, yi in zip(x, y))
        var_x = sum((xi - mean_x) ** 2 for xi in x)
        var_y = sum((yi - mean_y) ** 2 for yi in y)

        denom = math.sqrt(var_x * var_y)
        if denom == 0:
            return 0.0
        return round(cov / denom, 6)

    @staticmethod
    def spearman_correlation(x: list[float], y: list[float]) -> float:
        """Compute Spearman rank correlation coefficient.

        Returns a value in [-1, 1].  Returns 0.0 for lists with fewer than
        2 elements.
        """
        n = min(len(x), len(y))
        if n < 2:
            return 0.0

        x = x[:n]
        y = y[:n]

        rank_x = _rank(x)
        rank_y = _rank(y)

        return AnalyzeModule.pearson_correlation(rank_x, rank_y)

    @staticmethod
    def correlation_matrix(
        data: list[dict],
        fields: list[str],
        *,
        method: str = "pearson",
    ) -> dict[str, dict[str, float]]:
        """Compute pairwise correlation matrix for specified fields.

        Each dict in *data* should have all *fields* as numeric keys.

        Returns ``{field_a: {field_b: correlation, ...}, ...}``.
        """
        columns: dict[str, list[float]] = {}
        for f in fields:
            columns[f] = []
            for row in data:
                val = row.get(f)
                if val is not None:
                    try:
                        columns[f].append(float(val))
                    except (ValueError, TypeError):
                        continue

        corr_fn = (
            AnalyzeModule.spearman_correlation
            if method == "spearman"
            else AnalyzeModule.pearson_correlation
        )

        result: dict[str, dict[str, float]] = {}
        for fa in fields:
            result[fa] = {}
            for fb in fields:
                result[fa][fb] = corr_fn(columns.get(fa, []), columns.get(fb, []))
        return result

    @staticmethod
    def behavior_outcome_correlation(
        agents: list[dict],
        behavior_field: str,
        outcome_field: str,
    ) -> dict[str, Any]:
        """Analyze correlation between a behavior metric and an outcome.

        Each agent dict should have *behavior_field* and *outcome_field*.

        Returns pearson, spearman, sample size, and interpretation.
        """
        pairs: list[tuple[float, float]] = []
        for a in agents:
            b = a.get(behavior_field)
            o = a.get(outcome_field)
            if b is not None and o is not None:
                try:
                    pairs.append((float(b), float(o)))
                except (ValueError, TypeError):
                    continue

        if len(pairs) < 2:
            return {
                "pearson": 0.0,
                "spearman": 0.0,
                "sample_size": len(pairs),
                "interpretation": "insufficient data",
            }

        bx, oy = zip(*pairs)
        pearson = AnalyzeModule.pearson_correlation(list(bx), list(oy))
        spearman = AnalyzeModule.spearman_correlation(list(bx), list(oy))

        abs_p = abs(pearson)
        if abs_p >= 0.7:
            strength = "strong"
        elif abs_p >= 0.4:
            strength = "moderate"
        elif abs_p >= 0.2:
            strength = "weak"
        else:
            strength = "negligible"
        direction = "positive" if pearson > 0 else "negative" if pearson < 0 else "none"

        return {
            "pearson": pearson,
            "spearman": spearman,
            "sample_size": len(pairs),
            "interpretation": f"{strength} {direction} correlation",
        }

    # =====================================================================
    # Emergence Pattern Detection (3+ algorithms)
    # =====================================================================

    @staticmethod
    def detect_phase_transitions(
        time_series: list[dict],
        field: str,
        *,
        window: int = 10,
        threshold: float = 2.0,
    ) -> list[dict[str, Any]]:
        """Detect phase transitions using rolling z-score on differences.

        Algorithm 1: Rolling z-score on first differences.  Points where
        the absolute z-score exceeds *threshold* are flagged as transitions.

        Each dict should have ``tick`` and *field*.

        Returns list of ``{"tick", "field", "z_score", "change_direction"}``.
        """
        if len(time_series) < window + 1:
            return []

        ticks = [h.get("tick", i) for i, h in enumerate(time_series)]
        values = [float(h.get(field, 0)) for h in time_series]

        diffs = [values[i] - values[i - 1] for i in range(1, len(values))]
        transitions: list[dict[str, Any]] = []

        for i in range(window, len(diffs)):
            window_diffs = diffs[i - window:i]
            mean_d = sum(window_diffs) / window
            var_d = sum((d - mean_d) ** 2 for d in window_diffs) / window
            std_d = math.sqrt(var_d) if var_d > 0 else 0.0

            if std_d > 0:
                z = (diffs[i] - mean_d) / std_d
                if abs(z) > threshold:
                    transitions.append({
                        "tick": ticks[i + 1],
                        "field": field,
                        "z_score": round(z, 4),
                        "change_direction": "up" if z > 0 else "down",
                    })
            elif abs(diffs[i] - mean_d) > 0:
                # Zero variance window but current diff is non-zero: infinite z-score
                transitions.append({
                    "tick": ticks[i + 1],
                    "field": field,
                    "z_score": float("inf"),
                    "change_direction": "up" if diffs[i] > mean_d else "down",
                })

        return transitions

    @staticmethod
    def detect_clustering(
        agents: list[dict],
        fields: list[str],
        *,
        k: int = 3,
        max_iterations: int = 100,
    ) -> dict[str, Any]:
        """Detect emergent clusters using k-means on specified fields.

        Algorithm 2: K-means clustering on agent numeric fields.

        Returns cluster assignments, centroids, and cluster sizes.
        """
        # Extract numeric feature vectors
        vectors: list[tuple[list[float], str]] = []
        for a in agents:
            aid = str(a.get("id", ""))
            vec = []
            valid = True
            for f in fields:
                val = a.get(f)
                if val is None:
                    valid = False
                    break
                try:
                    vec.append(float(val))
                except (ValueError, TypeError):
                    valid = False
                    break
            if valid and vec:
                vectors.append((vec, aid))

        if len(vectors) < k:
            return {
                "clusters": [],
                "centroids": [],
                "cluster_sizes": [],
                "agent_assignments": {},
                "k": k,
            }

        # Initialize centroids using k-means++ style
        import random

        random.seed(42)  # deterministic for reproducibility
        dim = len(fields)
        centroids: list[list[float]] = []

        # Pick first centroid randomly
        first_idx = random.randint(0, len(vectors) - 1)
        centroids.append(vectors[first_idx][0][:])

        for _ in range(1, k):
            dists = []
            for vec, _ in vectors:
                min_dist = min(
                    sum((v - c) ** 2 for v, c in zip(vec, cen))
                    for cen in centroids
                )
                dists.append(min_dist)
            total_d = sum(dists)
            if total_d == 0:
                centroids.append(vectors[random.randint(0, len(vectors) - 1)][0][:])
                continue
            probs = [d / total_d for d in dists]
            cumulative = []
            cumsum = 0.0
            for p in probs:
                cumsum += p
                cumulative.append(cumsum)
            r = random.random()
            for idx, c in enumerate(cumulative):
                if r <= c:
                    centroids.append(vectors[idx][0][:])
                    break
            else:
                centroids.append(vectors[-1][0][:])

        # K-means iterations
        assignments: list[int] = [0] * len(vectors)
        for _ in range(max_iterations):
            changed = False
            for vi, (vec, _) in enumerate(vectors):
                best_c = 0
                best_dist = float("inf")
                for ci, cen in enumerate(centroids):
                    d = sum((v - c) ** 2 for v, c in zip(vec, cen))
                    if d < best_dist:
                        best_dist = d
                        best_c = ci
                if assignments[vi] != best_c:
                    changed = True
                    assignments[vi] = best_c

            if not changed:
                break

            # Update centroids
            sums: dict[int, list[float]] = {}
            counts: dict[int, int] = {}
            for ci in range(k):
                sums[ci] = [0.0] * dim
                counts[ci] = 0
            for vi, (vec, _) in enumerate(vectors):
                ci = assignments[vi]
                counts[ci] += 1
                for d in range(dim):
                    sums[ci][d] += vec[d]
            for ci in range(k):
                if counts[ci] > 0:
                    centroids[ci] = [sums[ci][d] / counts[ci] for d in range(dim)]

        # Build results
        cluster_agents: dict[int, list[str]] = defaultdict(list)
        agent_assignments: dict[str, int] = {}
        for vi, (vec, aid) in enumerate(vectors):
            cluster_agents[assignments[vi]].append(aid)
            agent_assignments[aid] = assignments[vi]

        clusters = [
            {"id": ci, "agents": sorted(cluster_agents.get(ci, [])), "size": len(cluster_agents.get(ci, []))}
            for ci in range(k)
        ]

        return {
            "clusters": clusters,
            "centroids": [[round(v, 4) for v in cen] for cen in centroids],
            "cluster_sizes": [len(cluster_agents.get(ci, [])) for ci in range(k)],
            "agent_assignments": agent_assignments,
            "k": k,
        }

    @staticmethod
    def detect_emergent_patterns(
        time_series: list[dict],
        fields: list[str],
        *,
        window: int = 20,
        variance_threshold: float = 3.0,
    ) -> dict[str, Any]:
        """Detect emergent patterns using multi-field variance analysis.

        Algorithm 3: Multi-field variance spike detection.  Identifies ticks
        where multiple fields simultaneously show anomalous variance.

        Each dict in *time_series* should have ``tick`` and all *fields*.

        Returns detected emergence events with contributing fields.
        """
        if len(time_series) < window + 1:
            return {"emergence_events": [], "field_anomalies": {}}

        ticks = [h.get("tick", i) for i, h in enumerate(time_series)]
        field_data: dict[str, list[float]] = {}
        for f in fields:
            field_data[f] = [float(h.get(f, 0)) for h in time_series]

        # Compute rolling z-scores per field
        field_anomalies: dict[str, list[dict[str, Any]]] = {}
        for f in fields:
            vals = field_data[f]
            anomalies: list[dict[str, Any]] = []
            for i in range(window, len(vals)):
                w = vals[i - window:i]
                mean_w = sum(w) / window
                std_w = math.sqrt(sum((v - mean_w) ** 2 for v in w) / window)
                if std_w > 0:
                    z = abs(vals[i] - mean_w) / std_w
                    if z > variance_threshold:
                        anomalies.append({
                            "tick": ticks[i],
                            "z_score": round(z, 4),
                            "value": vals[i],
                            "expected": round(mean_w, 4),
                        })
                elif abs(vals[i] - mean_w) > 0:
                    # Zero variance window but current value differs: infinite z-score
                    anomalies.append({
                        "tick": ticks[i],
                        "z_score": float("inf"),
                        "value": vals[i],
                        "expected": round(mean_w, 4),
                    })
            field_anomalies[f] = anomalies

        # Find simultaneous anomalies (emergence events)
        tick_scores: dict[int, dict[str, float]] = defaultdict(dict)
        for f, anomalies in field_anomalies.items():
            for a in anomalies:
                tick_scores[a["tick"]][f] = a["z_score"]

        emergence_events: list[dict[str, Any]] = []
        for tick in sorted(tick_scores):
            contributing = tick_scores[tick]
            if len(contributing) >= 2:  # Multi-field simultaneous anomaly
                emergence_events.append({
                    "tick": tick,
                    "contributing_fields": list(contributing.keys()),
                    "max_z_score": round(max(contributing.values()), 4),
                    "num_fields_anomalous": len(contributing),
                })

        return {
            "emergence_events": emergence_events,
            "field_anomalies": field_anomalies,
        }

    @staticmethod
    def detect_power_law(
        values: list[float],
    ) -> dict[str, Any]:
        """Detect power-law distribution using log-log regression.

        Algorithm 4: Checks whether the rank-frequency distribution of
        *values* follows a power law.  Fits log(rank) vs log(value) and
        reports the exponent and R-squared.

        Returns fit parameters and whether a power-law is plausible.
        """
        if len(values) < 10:
            return {
                "is_power_law": False,
                "exponent": 0.0,
                "r_squared": 0.0,
                "sample_size": len(values),
            }

        sorted_desc = sorted([v for v in values if v > 0], reverse=True)
        if len(sorted_desc) < 10:
            return {
                "is_power_law": False,
                "exponent": 0.0,
                "r_squared": 0.0,
                "sample_size": len(sorted_desc),
            }

        n = len(sorted_desc)
        log_ranks = [math.log(i + 1) for i in range(n)]
        log_vals = [math.log(v) for v in sorted_desc]

        # Linear regression on log-log
        slope, intercept, r_squared = _linear_regression(log_ranks, log_vals)

        # Power law is plausible if R^2 > 0.8 and slope < -0.5
        is_plausible = r_squared > 0.8 and slope < -0.5

        return {
            "is_power_law": is_plausible,
            "exponent": round(slope, 6),
            "intercept": round(intercept, 6),
            "r_squared": round(r_squared, 6),
            "sample_size": n,
        }

    # =====================================================================
    # Statistical Significance Testing
    # =====================================================================

    @staticmethod
    def t_test(
        group_a: list[float],
        group_b: list[float],
    ) -> dict[str, Any]:
        """Two-sample t-test (Welch's t-test) for comparing group means.

        Returns t_statistic, degrees_of_freedom, p_value_approx, and
        whether the difference is significant at alpha=0.05.
        """
        n_a = len(group_a)
        n_b = len(group_b)
        if n_a < 2 or n_b < 2:
            return {
                "t_statistic": 0.0,
                "df": 0,
                "p_value": 1.0,
                "significant_at_005": False,
            }

        mean_a = sum(group_a) / n_a
        mean_b = sum(group_b) / n_b
        var_a = sum((v - mean_a) ** 2 for v in group_a) / (n_a - 1)
        var_b = sum((v - mean_b) ** 2 for v in group_b) / (n_b - 1)

        se = math.sqrt(var_a / n_a + var_b / n_b)
        if se == 0:
            return {
                "t_statistic": 0.0,
                "df": n_a + n_b - 2,
                "p_value": 1.0,
                "significant_at_005": False,
            }

        t_stat = (mean_a - mean_b) / se

        # Welch-Satterthwaite degrees of freedom
        num = (var_a / n_a + var_b / n_b) ** 2
        denom = (var_a / n_a) ** 2 / (n_a - 1) + (var_b / n_b) ** 2 / (n_b - 1)
        df = num / denom if denom > 0 else n_a + n_b - 2

        # Approximate p-value using normal approximation for large df
        p_value = _approx_two_tailed_p(abs(t_stat), df)

        return {
            "t_statistic": round(t_stat, 6),
            "df": round(df, 2),
            "p_value": round(p_value, 6),
            "significant_at_005": p_value < 0.05,
        }

    @staticmethod
    def mann_whitney_u(
        group_a: list[float],
        group_b: list[float],
    ) -> dict[str, Any]:
        """Mann-Whitney U test for non-parametric comparison.

        Returns U statistic, z_score, and approximate p-value.
        """
        n_a = len(group_a)
        n_b = len(group_b)
        if n_a == 0 or n_b == 0:
            return {
                "u_statistic": 0,
                "z_score": 0.0,
                "p_value": 1.0,
                "significant_at_005": False,
            }

        # Rank all values together
        combined = [(v, "a", i) for i, v in enumerate(group_a)] + [
            (v, "b", i) for i, v in enumerate(group_b)
        ]
        combined.sort(key=lambda x: x[0])

        # Assign ranks with tie correction
        ranks: dict[tuple[str, int], float] = {}
        i = 0
        while i < len(combined):
            j = i
            while j < len(combined) and combined[j][0] == combined[i][0]:
                j += 1
            avg_rank = (i + 1 + j) / 2.0
            for k in range(i, j):
                ranks[(combined[k][1], combined[k][2])] = avg_rank
            i = j

        r1 = sum(ranks[("a", i)] for i in range(n_a))
        u1 = r1 - n_a * (n_a + 1) / 2
        u2 = n_a * n_b - u1
        u_stat = min(u1, u2)

        # Normal approximation
        mean_u = n_a * n_b / 2
        std_u = math.sqrt(n_a * n_b * (n_a + n_b + 1) / 12)
        z = (u_stat - mean_u) / std_u if std_u > 0 else 0.0
        p_value = 2 * (1 - _normal_cdf(abs(z)))

        return {
            "u_statistic": int(u_stat),
            "z_score": round(z, 6),
            "p_value": round(min(p_value, 1.0), 6),
            "significant_at_005": p_value < 0.05,
        }

    @staticmethod
    def chi_squared_test(
        observed: list[int],
        expected: list[float] | None = None,
    ) -> dict[str, Any]:
        """Chi-squared goodness-of-fit test.

        If *expected* is None, uniform distribution is assumed.

        Returns chi2_statistic, df, and approximate p-value.
        """
        n = len(observed)
        if n < 2:
            return {
                "chi2_statistic": 0.0,
                "df": 0,
                "p_value": 1.0,
                "significant_at_005": False,
            }

        total = sum(observed)
        if total == 0:
            return {
                "chi2_statistic": 0.0,
                "df": n - 1,
                "p_value": 1.0,
                "significant_at_005": False,
            }

        if expected is None:
            expected = [total / n] * n

        chi2 = sum(
            (o - e) ** 2 / e for o, e in zip(observed, expected) if e > 0
        )
        df = n - 1

        # Approximate p-value using chi-squared CDF
        p_value = 1.0 - _chi2_cdf(chi2, df)

        return {
            "chi2_statistic": round(chi2, 6),
            "df": df,
            "p_value": round(p_value, 6),
            "significant_at_005": p_value < 0.05,
        }

    # =====================================================================
    # Cultural Diversity (existing)
    # =====================================================================

    def cultural_diversity(self, data: list[dict]) -> dict[str, Any]:
        """Compute cultural diversity metrics from a list of agent profiles.

        Each agent dict should have a ``phase`` key (and optionally others).

        Returns a dict with:
        - ``shannon_entropy`` — entropy of the phase distribution
        - ``unique_phases`` — number of distinct phases
        - ``phase_counts`` — dict of phase -> count
        - ``simpson_index`` — 1 - sum(p_i^2), probability that two randomly
          chosen agents belong to different phases
        """
        if not data:
            return {
                "shannon_entropy": 0.0,
                "unique_phases": 0,
                "phase_counts": {},
                "simpson_index": 0.0,
            }

        phase_counts: Counter[str] = Counter(
            agent.get("phase", "unknown") for agent in data
        )
        total = len(data)
        unique = len(phase_counts)

        # Shannon entropy
        entropy = 0.0
        for count in phase_counts.values():
            p = count / total
            if p > 0:
                entropy -= p * math.log2(p)

        # Simpson diversity index (1 - D)
        d = sum((c / total) ** 2 for c in phase_counts.values())
        simpson = 1.0 - d

        return {
            "shannon_entropy": round(entropy, 4),
            "unique_phases": unique,
            "phase_counts": dict(phase_counts),
            "simpson_index": round(simpson, 4),
        }

    def cultural_evolution(
        self,
        snapshots: list[dict],
        field: str = "phase",
    ) -> dict[str, Any]:
        """Analyze cultural evolution across time snapshots.

        Each snapshot should have ``tick`` and a list of agents (or be
        a flat list of agent dicts each with ``tick``).

        Returns entropy trajectory, diversity trajectory, and transition matrix.
        """
        if not snapshots:
            return {
                "entropy_trajectory": [],
                "diversity_trajectory": [],
                "dominant_trait_trajectory": [],
            }

        # Group agents by tick
        by_tick: dict[int, list[dict]] = defaultdict(list)
        for s in snapshots:
            tick = s.get("tick", 0)
            agents = s.get("agents", [])
            if not agents:
                # The snapshot itself is an agent
                by_tick[tick].append(s)
            else:
                by_tick[tick].extend(agents)

        entropy_traj: list[dict[str, Any]] = []
        diversity_traj: list[dict[str, Any]] = []
        dominant_traj: list[dict[str, Any]] = []

        for tick in sorted(by_tick):
            agents = by_tick[tick]
            if not agents:
                continue
            counts = Counter(str(a.get(field, "unknown")) for a in agents)
            n = len(agents)

            entropy = 0.0
            for c in counts.values():
                p = c / n
                if p > 0:
                    entropy -= p * math.log2(p)

            simpson = 1.0 - sum((c / n) ** 2 for c in counts.values())
            dominant = counts.most_common(1)[0]

            entropy_traj.append({"tick": tick, "entropy": round(entropy, 4)})
            diversity_traj.append({"tick": tick, "simpson_index": round(simpson, 4)})
            dominant_traj.append({
                "tick": tick,
                "dominant_trait": dominant[0],
                "dominant_fraction": round(dominant[1] / n, 4),
            })

        return {
            "entropy_trajectory": entropy_traj,
            "diversity_trajectory": diversity_traj,
            "dominant_trait_trajectory": dominant_traj,
        }

    # =====================================================================
    # Network Analysis Helpers (existing + new)
    # =====================================================================

    def trust_network(self, edges: list[dict]) -> dict[str, Any]:
        """Analyse a trust / interaction network from an edge list.

        Each edge dict should have ``source``, ``target``, and optionally
        ``weight``.

        Returns:
        - ``node_count`` — number of unique agents
        - ``edge_count`` — number of edges
        - ``avg_degree`` — average connections per node
        - ``density`` — edge_count / (node_count * (node_count - 1)) for directed
        """
        if not edges:
            return {
                "node_count": 0,
                "edge_count": 0,
                "avg_degree": 0.0,
                "density": 0.0,
            }

        nodes: set[str] = set()
        for edge in edges:
            nodes.add(str(edge.get("source", "")))
            nodes.add(str(edge.get("target", "")))
        nodes.discard("")

        node_count = len(nodes)
        edge_count = len(edges)
        avg_degree = (edge_count / node_count) if node_count > 0 else 0.0
        density = (
            edge_count / (node_count * (node_count - 1))
            if node_count > 1
            else 0.0
        )

        return {
            "node_count": node_count,
            "edge_count": edge_count,
            "avg_degree": round(avg_degree, 4),
            "density": round(density, 6),
        }

    # =====================================================================
    # Agent Behavior Trajectory
    # =====================================================================

    @staticmethod
    def agent_trajectory(
        events: list[dict],
        agent_id: str,
    ) -> dict[str, Any]:
        """Build a decision trajectory for a specific agent.

        Each event should have ``agent_id``, ``tick``, ``action``, and
        optionally ``phase``, ``outcome``.

        Returns the sorted action sequence, tick range, and phase transitions.
        """
        agent_events = sorted(
            [e for e in events if str(e.get("agent_id", "")) == agent_id],
            key=lambda e: e.get("tick", 0),
        )

        if not agent_events:
            return {
                "agent_id": agent_id,
                "action_sequence": [],
                "tick_range": None,
                "phase_transitions": [],
                "total_events": 0,
            }

        action_seq = [e.get("action", "unknown") for e in agent_events]
        ticks = [e.get("tick", 0) for e in agent_events]

        # Phase transitions
        phases = [e.get("phase", "unknown") for e in agent_events]
        transitions: list[dict[str, Any]] = []
        for i in range(1, len(phases)):
            if phases[i] != phases[i - 1]:
                transitions.append({
                    "tick": ticks[i],
                    "from_phase": phases[i - 1],
                    "to_phase": phases[i],
                })

        return {
            "agent_id": agent_id,
            "action_sequence": action_seq,
            "tick_range": [ticks[0], ticks[-1]],
            "phase_transitions": transitions,
            "total_events": len(agent_events),
        }

    @staticmethod
    def decision_tree(
        events: list[dict],
        agent_id: str,
    ) -> dict[str, Any]:
        """Build a decision tree from agent events.

        Each event should have ``agent_id``, ``tick``, ``action``,
        ``context`` (optional), and ``outcome`` (optional).

        Returns a tree structure with decisions, contexts, and outcomes.
        """
        agent_events = sorted(
            [e for e in events if str(e.get("agent_id", "")) == agent_id],
            key=lambda e: e.get("tick", 0),
        )

        if not agent_events:
            return {"agent_id": agent_id, "nodes": [], "depth": 0}

        nodes: list[dict[str, Any]] = []
        for i, e in enumerate(agent_events):
            node: dict[str, Any] = {
                "tick": e.get("tick", 0),
                "action": e.get("action", "unknown"),
                "depth": i,
            }
            if "context" in e:
                node["context"] = e["context"]
            if "outcome" in e:
                node["outcome"] = e["outcome"]
            nodes.append(node)

        return {
            "agent_id": agent_id,
            "nodes": nodes,
            "depth": len(nodes),
        }

    # =====================================================================
    # Economic Time Series Helpers
    # =====================================================================

    @staticmethod
    def economic_time_series(
        history: list[dict],
        fields: list[str] | None = None,
    ) -> dict[str, Any]:
        """Extract and analyze economic time series data.

        Each snapshot should have ``tick`` and numeric fields like
        ``total_money``, ``total_tokens``, ``gdp``, ``gini_coefficient``.

        Returns per-field time series with trend info.
        """
        if not history:
            return {"series": {}, "tick_count": 0}

        if fields is None:
            fields = ["total_money", "total_tokens", "gdp", "gini_coefficient"]

        ticks = [h.get("tick", i) for i, h in enumerate(history)]

        series: dict[str, dict[str, Any]] = {}
        for f in fields:
            values = [float(h.get(f, 0)) for h in history]
            if all(v == 0 for v in values):
                continue

            slope, intercept, r_sq = _linear_regression(
                list(range(len(values))), values
            )

            first = values[0] if values else 0
            last = values[-1] if values else 0
            change_pct = ((last - first) / first * 100) if first else 0.0

            series[f] = {
                "values": values,
                "min": min(values),
                "max": max(values),
                "mean": round(sum(values) / len(values), 4),
                "change_pct": round(change_pct, 4),
                "slope": round(slope, 6),
                "r_squared": round(r_sq, 6),
            }

        return {
            "series": series,
            "ticks": ticks,
            "tick_count": len(history),
        }

    # =====================================================================
    # Composite / Convenience
    # =====================================================================

    @staticmethod
    def world_summary(
        agents: list[dict],
        history: list[dict] | None = None,
    ) -> dict[str, Any]:
        """Generate a comprehensive world analysis summary.

        Combines population stats, wealth distribution, skill diversity,
        and optional time series trends.
        """
        n = len(agents)
        if n == 0:
            return {"population": 0}

        alive = [a for a in agents if a.get("alive", True)]
        phases = Counter(str(a.get("phase", "unknown")) for a in agents)

        money_vals = [float(a.get("money", 0)) for a in alive] if alive else [0]
        token_vals = [float(a.get("tokens", 0)) for a in alive] if alive else [0]

        # Skill diversity
        all_skills: Counter[str] = Counter()
        for a in alive:
            skills = a.get("skills", {})
            if isinstance(skills, dict):
                all_skills.update(skills.keys())

        summary: dict[str, Any] = {
            "population": n,
            "alive_count": len(alive),
            "dead_count": n - len(alive),
            "survival_rate": round(len(alive) / n, 4),
            "phase_distribution": dict(phases),
            "wealth": AnalyzeModule.descriptive_stats(money_vals),
            "tokens": AnalyzeModule.descriptive_stats(token_vals),
            "skill_diversity": len(all_skills),
            "top_skills": all_skills.most_common(10),
        }

        if history:
            summary["economic_trend"] = AnalyzeModule.economic_time_series(history)

        return summary


# =========================================================================
# Module-level helpers
# =========================================================================

def _empty_descriptive() -> dict[str, Any]:
    return {
        "count": 0, "mean": 0.0, "median": 0.0, "mode": 0.0,
        "variance": 0.0, "std_dev": 0.0, "min": 0.0, "max": 0.0,
        "range": 0.0, "skewness": 0.0, "kurtosis": 0.0,
        "q1": 0.0, "q3": 0.0, "iqr": 0.0,
    }


def _percentile(sorted_vals: list[float], pct: float) -> float:
    """Compute percentile from pre-sorted values."""
    n = len(sorted_vals)
    if n == 0:
        return 0.0
    k = (n - 1) * pct / 100.0
    f = math.floor(k)
    c = math.ceil(k)
    if f == c:
        return sorted_vals[int(k)]
    return sorted_vals[int(f)] * (c - k) + sorted_vals[int(c)] * (k - f)


def _rank(values: list[float]) -> list[float]:
    """Assign ranks to values, averaging ties."""
    indexed = sorted(enumerate(values), key=lambda x: x[1])
    ranks = [0.0] * len(values)
    i = 0
    while i < len(indexed):
        j = i
        while j < len(indexed) and indexed[j][1] == indexed[i][1]:
            j += 1
        avg_rank = (i + 1 + j) / 2.0
        for k in range(i, j):
            ranks[indexed[k][0]] = avg_rank
        i = j
    return ranks


def _linear_regression(
    xs: list[float], ys: list[float]
) -> tuple[float, float, float]:
    """Simple linear regression.  Returns (slope, intercept, r_squared)."""
    n = len(xs)
    if n < 2:
        return 0.0, 0.0, 0.0

    sx = sum(xs)
    sy = sum(ys)
    sxy = sum(x * y for x, y in zip(xs, ys))
    sx2 = sum(x * x for x in xs)

    denom = n * sx2 - sx * sx
    if denom == 0:
        return 0.0, 0.0, 0.0

    slope = (n * sxy - sx * sy) / denom
    intercept = (sy - slope * sx) / n

    mean_y = sy / n
    ss_tot = sum((y - mean_y) ** 2 for y in ys)
    ss_res = sum((y - (slope * x + intercept)) ** 2 for x, y in zip(xs, ys))
    r_squared = 1 - ss_res / ss_tot if ss_tot > 0 else 0.0

    return slope, intercept, r_squared


def _normal_cdf(z: float) -> float:
    """Approximate the standard normal CDF using error function."""
    return 0.5 * (1.0 + math.erf(z / math.sqrt(2.0)))


def _approx_two_tailed_p(t_stat: float, df: float) -> float:
    """Approximate two-tailed p-value from t-distribution.

    Uses normal approximation for large df, and a simple correction
    for smaller df.
    """
    z = t_stat / math.sqrt(1.0 + t_stat * t_stat / df) if df > 0 else t_stat
    p = 2.0 * (1.0 - _normal_cdf(abs(z)))
    return min(p, 1.0)


def _chi2_cdf(x: float, df: int) -> float:
    """Approximate chi-squared CDF using regularized incomplete gamma."""
    if df <= 0 or x <= 0:
        return 0.0
    return _regularized_gamma_lower(df / 2.0, x / 2.0)


def _regularized_gamma_lower(a: float, x: float) -> float:
    """Regularized lower incomplete gamma function P(a, x).

    Uses series expansion for small x, continued fraction for large x.
    """
    if x < a + 1:
        # Series expansion
        total = 1.0 / a
        term = 1.0 / a
        for n in range(1, 200):
            term *= x / (a + n)
            total += term
            if abs(term) < abs(total) * 1e-10:
                break
        log_gamma = _log_gamma(a)
        return total * math.exp(-x + a * math.log(x) - log_gamma)
    else:
        # Continued fraction
        log_gamma = _log_gamma(a)
        b = x + 1.0 - a
        c = 1.0 / 1e-30
        d = 1.0 / b
        h = d
        for i in range(1, 200):
            an = -i * (i - a)
            b += 2.0
            d = an * d + b
            if abs(d) < 1e-30:
                d = 1e-30
            c = b + an / c
            if abs(c) < 1e-30:
                c = 1e-30
            d = 1.0 / d
            delta = d * c
            h *= delta
            if abs(delta - 1.0) < 1e-10:
                break
        return 1.0 - h * math.exp(-x + a * math.log(x) - log_gamma)


def _log_gamma(x: float) -> float:
    """Log of the gamma function using Lanczos approximation."""
    if x <= 0:
        return 0.0
    coefs = [
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
    for c in coefs:
        y += 1.0
        ser += c / y
    return -tmp + math.log(2.5066282746310005 * ser / x)
