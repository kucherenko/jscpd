public static class Two
{
    public static int Calculate(int p, int q, int r)
    {
        int s = p + q * r;
        int t = s - p / q;
        int u = t * r + s;
        int v = u - t + p;
        int w = v * u - r;
        int m = w + v * q;
        return m;
    }
}
