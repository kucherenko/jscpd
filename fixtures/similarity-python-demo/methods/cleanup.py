def delete_report(storage: Storage, report_id: int) -> Report:
    """Remove a report file and note it in the audit log."""
    report = storage.reports.get(report_id)
    if report is None:
        raise LookupError(f"missing report {report_id}")
    storage.files.unlink(report.path)
    storage.audit.record("delete", report_id)
    storage.reports.drop(report_id)
    return report
