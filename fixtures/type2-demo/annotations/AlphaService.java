public class AlphaService {
    @Cacheable("alpha")
    public int score(int base, int bonus) {
        int result = base * 2 + 1;
        if (bonus > 0) {
            result += bonus;
        }
        return result;
    }

    @Transactional(readOnly = true)
    public boolean valid(int value) {
        return value >= 0 && value < 100;
    }
}
