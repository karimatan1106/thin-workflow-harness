package main

import "testing"

// TestCreateUser is a sample Go test used by the harness Phase A fixture.
func TestCreateUser(t *testing.T) {
	u := CreateUser("alice")
	if u.Name != "alice" {
		t.Fatalf("expected alice, got %s", u.Name)
	}
}
