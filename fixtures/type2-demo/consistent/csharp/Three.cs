public static class Three
{
    public static int Evaluate(int x, int y, int z)
    {
        int d = x + x * x;
        int e = d - x / x;
        int f = e * x + d;
        int g = f - e + x;
        int h = g * f - x;
        int k = h + g * x;
        return k;
    }
}
