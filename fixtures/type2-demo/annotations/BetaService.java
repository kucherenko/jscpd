public class BetaService {
    @Deprecated
    public int score(int base, int bonus) {
        int result = base * 2 + 1;
        if (bonus > 0) {
            result += bonus;
        }
        return result;
    }

    @Override
    public boolean valid(int value) {
        return value >= 0 && value < 100;
    }
}
