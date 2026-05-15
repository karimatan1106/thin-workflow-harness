import { User } from "./user.js";

export function test_create_user(): void {
  const u = User.create("alice");
  if (u.name !== "alice") throw new Error("fail");
}
