# hello


def hello():
    print("hello")


def hello1():
    print("hello")


def hello3():
    print("hello")


class A(object):
    def __init__(self):
        print("qwe")
        self.test = None
        if self.test:
            print(self.test)

    def hello4(self):
        pass


if __name__ == "__main__":
    a = A()
