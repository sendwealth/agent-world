# Emergence Benchmark Report — park-benchmark

- **Generated:** 2026-06-15T00:12:45.216854+00:00
- **Schema:** `emergence-benchmark/v1`
- **Metrics:** 6
- **Agents:** 25  **Ticks:** 200  **Seed:** 42

## Reproduction criteria (Park et al. 2023)

- [PASS] information_diffusion_coverage_>=50%
- [PASS] social_network_density_>=0.05
- [PASS] role_specialization_mean_>_0

## 1. Information Diffusion

- **total_population:** 25
- **final_informed:** 25
- **final_coverage:** 1.0
- **adoption_rate:** 0.151533
- **half_life_tick:** 24.5
- **ticks_to_90pct:** 39
- **mean_first_seen_tick:** 23.28

> Coverage: 100.0% of population acquired the information. Adoption rate r = 0.1515. 90% coverage reached at tick 39.

## 2. Social Network

- **node_count:** 25
- **edge_count:** 181
- **density:** 0.603333
- **global_clustering_coefficient:** 0.604781
- **mean_degree:** 14.48
- **largest_component_ratio:** 1.0

> Network density = 0.6033 (dense, comparable to Smallville). Global clustering coefficient = 0.6048. Largest component covers 100.0% of agents.

## 3. Role Specialization

- **agent_count:** 25
- **role_count:** 5
- **mean_specialization:** 0.340756
- **role_diversity_entropy:** 2.280755
- **role_diversity_normalized:** 0.982268
- **top_role_share:** 0.2694

> Mean per-agent specialization = 0.3408 (1.0 = each agent does one role). Population role diversity (normalised) = 0.9823 across 5 roles.

## 4. Economic Inequality

- **tick_count:** 11
- **mean_gini:** 0.052055
- **final_gini:** 0.090144
- **gini_trend_slope:** 0.000373
- **final_top10_share:** 0.152948

> Final Gini = 0.0901 (low, increasing); top-10% holds 15.3% of wealth.

## 5. Organization Stability

- **total_orgs_formed:** 15
- **orgs_alive_at_end:** 7
- **mean_lifespan_ticks:** 76.866667
- **median_lifespan_ticks:** 72
- **churn_rate:** 0.533333
- **mean_peak_members:** 3.266667

> 15 orgs formed; 7 still active (churn = 0.53). Mean lifespan = 76.9 ticks. (38% of run).

## 6. Cultural Diversity

- **tick_count:** 11
- **mean_entropy:** 1.955254
- **mean_normalized_entropy:** 0.977627
- **final_entropy:** 1.957406
- **signal_categories:** 4

> Mean normalised cultural entropy = 0.9776 (highly diverse) across up to 4 categories.

