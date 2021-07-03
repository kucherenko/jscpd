import a from 'a';
import b from 'b';
import c from 'c';
import d from 'd';
import e from 'e';
import f from 'f';

const USER_1 = Symbol('USER_1');
const USER_2 = Symbol('USER_2');
const USER_3 = Symbol('USER_3');
const USER_4 = Symbol('USER_4');
const USER_5 = Symbol('USER_5');
const USER_6 = Symbol('USER_6');

function fibonacci(num, memo) {
    memo = memo || {};
  
    if (memo[num]) return memo[num];
    if (num <= 1) return 1;
  
    return memo[num] = fibonacci(num - 1, memo) + fibonacci(num - 2, memo);
}

export { fibonacci }
