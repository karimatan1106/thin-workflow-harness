package main

import "testing"

func TestMore(t *testing.T) {
	_ = CreateUser("more")
}

func BenchmarkCreate(b *testing.B) {
	for i := 0; i < b.N; i++ {
		_ = CreateUser("bench")
	}
}

func ExampleCreateUser() {
	_ = CreateUser("example")
}

func FuzzCreate(f *testing.F) {
	f.Add("seed")
	f.Fuzz(func(t *testing.T, name string) {
		_ = CreateUser(name)
	})
}
