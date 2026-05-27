//! Test report types and formatting.

use colored::*;

#[derive(Debug, Clone)]
pub enum TestStatus {
    Passed,
    Failed(String),
    Skipped(String),
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct TestReport {
    pub results: Vec<TestResult>,
    pub plugin_name: String,
}

impl TestReport {
    pub fn new(plugin_name: &str) -> Self {
        Self {
            results: vec![],
            plugin_name: plugin_name.into(),
        }
    }

    pub fn add(&mut self, result: TestResult) {
        self.results.push(result);
    }

    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| matches!(r.status, TestStatus::Passed)).count()
    }

    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| matches!(r.status, TestStatus::Failed(_))).count()
    }

    pub fn skipped_count(&self) -> usize {
        self.results.iter().filter(|r| matches!(r.status, TestStatus::Skipped(_))).count()
    }

    pub fn all_passed(&self) -> bool {
        self.failed_count() == 0
    }

    pub fn print(&self) {
        println!();
        println!("{}", format!("🧪 Test Report: {}", self.plugin_name).bold());
        println!("{}", "─".repeat(50));

        for result in &self.results {
            let status_str = match &result.status {
                TestStatus::Passed => "✅ PASS".green().to_string(),
                TestStatus::Failed(msg) => format!("❌ FAIL — {}", msg).red().to_string(),
                TestStatus::Skipped(reason) => format!("⏭️  SKIP — {}", reason).yellow().to_string(),
            };
            println!(
                "  {} {} ({})",
                status_str,
                result.name,
                format!("{}ms", result.duration_ms).dimmed()
            );
        }

        println!("{}", "─".repeat(50));
        let total = self.results.len();
        let summary = format!(
            "{} passed, {} failed, {} skipped ({} total)",
            self.passed_count().to_string().green(),
            self.failed_count().to_string().red(),
            self.skipped_count().to_string().yellow(),
            total,
        );
        println!("{}", summary.bold());

        if self.all_passed() {
            println!("{}", "All tests passed! 🎉".green().bold());
        } else {
            println!("{}", "Some tests failed.".red().bold());
        }
    }
}
