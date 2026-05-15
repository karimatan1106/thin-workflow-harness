from .user import User, create_user


def main() -> None:
    u = create_user("alice")
    print(u.name)
