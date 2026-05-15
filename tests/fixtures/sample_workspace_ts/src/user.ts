export class User {
  static create(name: string): User {
    return new User(name);
  }
  private constructor(public name: string) {}
}
