
import Foundation

class SwiftMath : NSObject
{
    var name : String?
    init(name: String)
    {
        self.name = name

        println(self.name)
    }

    func sum(num1 :Int, num2 :Int) -> Int
    {
        return (num1 + num2)
    }

    func multiply(num1: Int, num2 :Int) -> Int
    {
        return num1 * num2
    }

    func multiply_1(num1: Int, num2 :Int) -> Int
    {
        return num1 * num2
    }
}
