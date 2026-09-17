// Left behind when tracking moved to the new API. Nothing loads it, and no
// glob or alias reaches this directory.
export function fetchCourierStatus(code) {
  return fetch(`/api/v1/courier/${code}`).then((response) => response.json());
}
