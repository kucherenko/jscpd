def export_report(storage: Storage, report_id: int) -> Report:
    """Upload a report and note it in the audit log."""
    report = storage.reports.get(report_id)
    if report is None:
        raise LookupError(f"no report {report_id}")
    storage.files.upload(report.path)
    storage.audit.record("export", report_id)
    storage.reports.touch(report_id)
    return report
