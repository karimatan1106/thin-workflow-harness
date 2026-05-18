package main

// User represents a sample user record.
type User struct {
	Name string
	Age  int
}

// CreateUser constructs a new User with the given name.
func CreateUser(name string) *User {
	return &User{Name: name, Age: 0}
}
