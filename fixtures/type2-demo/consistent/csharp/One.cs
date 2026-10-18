public static class One
{
    public static int Compute(int a, int b, int c)
    {
        int d = a + b * c;
        int e = d - a / b;
        int f = e * c + d;
        int g = f - e + a;
        int h = g * f - c;
        int k = h + g * b;
        return k;
    }
}
