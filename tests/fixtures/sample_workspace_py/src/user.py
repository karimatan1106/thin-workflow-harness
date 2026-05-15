class User:
    def __init__(self, name: str) -> None:
        self.name = name


def create_user(name: str) -> User:
    return User(name)
