import "console.jsx";

class Point {
  var x : number;
  var y : number;
  function constructor(x : number, y : number) {
    this.x = x;
    this.y = y;
  }
}

class Matrix {
  var m11 : number;
  var m12 : number;
  var m13 : number;
  var m21 : number;
  var m22 : number;
  var m23 : number;

  function constructor(m11 : number, m12 : number, m13 : number, m21 : number, m22 : number, m23 : number) {
    this.m11 = m11;
    this.m12 = m12;
    this.m13 = m13;
    this.m21 = m21;
    this.m22 = m22;
    this.m23 = m23;
  }

  function transform(pt : Point) : Point {
    return new Point(
        this.m11 * pt.x + this.m12 * pt.y + this.m13,
        this.m21 * pt.x + this.m22 * pt.y + this.m22);
  }
}

class Matrix {
  var m11 : number;
  var m12 : number;
  var m13 : number;
  var m21 : number;
  var m22 : number;
  var m23 : number;

  function constructor(m11 : number, m12 : number, m13 : number, m21 : number, m22 : number, m23 : number) {
    this.m11 = m11;
    this.m12 = m12;
    this.m13 = m13;
    this.m21 = m21;
    this.m22 = m22;
    this.m23 = m23;
  }

  function transform(pt : Point) : Point {
    return new Point(
        this.m11 * pt.x + this.m12 * pt.y + this.m13,
        this.m21 * pt.x + this.m22 * pt.y + this.m22);
  }
}

class _Main {
  static function main(args : string[]) : void {
    var x = new Matrix(1, 0, 0, 0, 2, 0).transform(new Point(1, 0));
    console.log(x);
    var x = new Matrix(1, 0, 0, 0, 2, 0).transform(new Point(2, 1));
    console.log(x);
  }
}

// vim: set tabstop=2 shiftwidth=2 expandtab:
