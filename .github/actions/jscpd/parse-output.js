const fs = require("fs");
const path = require("path");

const outputDir = process.argv[2] || "report";
const reportName = process.argv[3] || "jscpd-report";
const reportPath = path.join(outputDir, `${reportName}.json`);
const sarifPath = path.join(outputDir, `${reportName}.sarif`);

const defaults = {
  "duplication-percentage": "0",
  "clones-found": "0",
  "duplicated-lines": "0",
  "total-lines": "0",
  "files-count": "0",
  "report-path": outputDir,
  "sarif-path": "",
};

function setOutput(name, value) {
  const filePath = process.env.GITHUB_OUTPUT;
  if (filePath) {
    fs.appendFileSync(filePath, `${name}=${value}\n`);
  } else {
    console.log(`${name}=${value}`);
  }
}

try {
  if (!fs.existsSync(reportPath)) {
    console.log(`jscpd report not found at ${reportPath}, using defaults`);
    // Not silent: a reportName in .jscpd.json renames the files this step
    // reads, and the outputs would be zeros.
    console.log(
      `::warning::jscpd report not found at ${reportPath}: the outputs are zeros. ` +
        "If .jscpd.json sets reportName, set the report-name input to the same name.",
    );
    for (const [name, value] of Object.entries(defaults)) {
      setOutput(name, value);
    }
    process.exit(0);
  }

  const raw = fs.readFileSync(reportPath, "utf8");
  const report = JSON.parse(raw);
  const total = report.statistics && report.statistics.total;

  if (!total) {
    console.log("No statistics.total in report, using defaults");
    for (const [name, value] of Object.entries(defaults)) {
      setOutput(name, value);
    }
    process.exit(0);
  }

  setOutput("duplication-percentage", String(total.percentage ?? 0));
  setOutput("clones-found", String(total.clones ?? 0));
  setOutput("duplicated-lines", String(total.duplicatedLines ?? 0));
  setOutput("total-lines", String(total.lines ?? 0));
  setOutput("files-count", String(total.sources ?? 0));
  setOutput("report-path", outputDir);
  setOutput("sarif-path", fs.existsSync(sarifPath) ? sarifPath : "");
} catch (err) {
  console.log(`Error parsing jscpd report: ${err.message}`);
  for (const [name, value] of Object.entries(defaults)) {
    setOutput(name, value);
  }
  process.exit(0);
}