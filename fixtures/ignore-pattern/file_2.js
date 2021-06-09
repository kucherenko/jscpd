import a from 'a';
import b from 'b';
import c from 'c';
import d from 'd';
import e from 'e';
import f from 'f';

const DELIM = Symbol('|');

const USER_1 = Symbol('USER_1');
const USER_2 = Symbol('USER_2');
const USER_3 = Symbol('USER_3');
const USER_4 = Symbol('USER_4');
const USER_5 = Symbol('USER_5');

export function factorial(n){
    let answer = 1;
    if (n == 0 || n == 1){
      return answer;
    }else{
      for(var i = n; i >= 1; i--){
        answer = answer * i;
      }
      return answer;
    }  
}