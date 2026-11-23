#!/usr/bin/env python3
"""Verify the loopback demo from another terminal while it is running."""
import json
import urllib.request
import urllib.error

base = "http://127.0.0.1:8000/api/v1/quizzes/"
quiz = "00000000-0000-4000-8000-000000000100"

def get(user=None, resource=quiz):
    request = urllib.request.Request(base + resource + "/my-results")
    if user:
        request.add_header("Authorization", "Bearer demo-" + user)
    try:
        response = urllib.request.urlopen(request, timeout=10)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        assert response.headers.get("x-request-id"), "missing request ID"
        return response.status, json.load(response)

status, a = get("a")
assert status == 200 and len(a["attempts"]) == 2
assert [x["percentage"] for x in a["attempts"]] == [70, 90]
assert a["revision_results"][0]["best_percentage"] == 90
assert a["revision_results"][0]["rank"] == 1
assert get("b")[1]["revision_results"][0]["rank"] == 1
assert get("c")[1]["revision_results"][0]["rank"] == 3
assert get("d")[1]["revision_results"][0]["quiz_revision_number"] == 2
assert get("z")[1]["attempts"] == []
assert get()[0] == 401
assert get("outsider")[0] == 404
assert get("new-member")[0] == 404
assert get("a", "invalid")[0] == 400
print("PASS: live HTTP demo, history, ties, revisions, empty history, auth and access checks")
