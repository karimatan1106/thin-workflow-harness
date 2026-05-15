import pytest

from .user import create_user


def test_create_user():
    user = create_user("alice")
    assert user.name == "alice"


@pytest.mark.parametrize("name", ["alice", "bob"])
def test_user_name(name):
    user = create_user(name)
    assert user.name == name


@pytest.fixture
def setup_user():
    return create_user("seed")


class TestUser:
    def test_method(self):
        user = create_user("clara")
        assert user.name == "clara"

    def helper(self):
        return 42
